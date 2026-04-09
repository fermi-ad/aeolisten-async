use clap::Parser;
use colored::Colorize;
use redis::{AsyncCommands, Client, RedisResult, streams::StreamMaxlen};
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::net::UdpSocket;

mod edp;

type Alarm = [(String, String); 6];

async fn aeolus_task(sndr: Sender<Alarm>, mcast_addr: String, listen_port: u16) -> std::io::Result<()>
{
  let sock2 = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

  let _ = sock2.set_reuse_address(true);

  let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, listen_port);

  let _ = sock2.bind(&addr.into());

  let group: Ipv4Addr = mcast_addr.parse().map_err(|e| std::io::Error::other(e))?;

  let _ = sock2.join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED);

  let sock = UdpSocket::from_std(sock2.into())?;

  println!("{}", format!("\nListening on port {listen_port} to multicast from address {mcast_addr}").white());

  let mut buf = [0u8; 9999];

  loop
  {
    let (len, _) = sock.recv_from(&mut buf).await?;

    if len == buf.len() { println!("{}", "\nMax data received".bright_red()); }

    println!("{}", format!("\nReceived {len} bytes").white());

    if len < 32
    {
      println!("{}", "\nHeader too short".bright_red());
      continue;
    }

    let ip_ver  = buf[ 0] as f32 + buf[ 1] as f32 / 10.0;
    let mc_ver  = buf[20] as f32 + buf[21] as f32 / 10.0;
    let seq_num = edp::be_u32(&buf, 24);

    if len == 32+28
    {
      let hb = &buf[32..];

      let typecode    = hb[0];
      let seconds    = edp::be_u32(&hb, 4);
      let edm_seq    = edp::be_u32(&hb, 8);
      let evt_seq    = edp::be_u32(&hb, 12);
      let evt_num    = edp::be_u32(&hb, 16);
      let hb_seq     = edp::be_u32(&hb, 20);
      let count      = edp::be_u32(&hb, 24);

      println!("{}", format!("HB  ip_ver: {ip_ver:.1}   mc_ver: {mc_ver:.1}   seq_num: {seq_num}   typecode: {typecode}   seconds: {seconds}").green());
      println!("{}", format!("    edm_seq: {edm_seq}    evt_seq: {evt_seq}    evt_num: {evt_num}   hb_seq: {hb_seq:4}   count: {count:3}").green());
    }
    else if len == 32+72
    {
      let mclr = &buf[32..];

      let typecode   = mclr[0];
      let daemon_id = edp::be_u32(&mclr, 64);
      let edm_seq   = edp::be_u32(&mclr, 68);

      println!("{}", format!("MCLR  ip_ver: {ip_ver:.1}   mc_ver: {mc_ver:.1}   seq_num: {seq_num}   typecode: {typecode}   daemon_id: {daemon_id}   edm_seq: {edm_seq}").magenta());
    }
    else
    {
      let hdr = &buf[32..];

      let typecode  = hdr[0];
      let count     = hdr[1];
      let version   = hdr[2];
      let edm_seq  = edp::be_u32(&hdr, 4);

      println!("{}", format!("EDP  ip_ver: {ip_ver:.1}   mc_ver: {mc_ver:.1}   seq_num: {seq_num}   typecode: {typecode}   count: {count}   version: {version}   edm_seq: {edm_seq}").green());

      let mut rec = &hdr[8..];

      for _ in 0..count
      {
        let edp = edp::EDP::new(rec);

        println!();
        edp.println();

        let source   = if edp.is_digital() { "DIGITAL" } else { "ANALOG"};
        let severity = if edp.alarm() == 0 { "NO_ALARM" } else if edp.priority < 10 { "MINOR" } else { "MAJOR" };

        let detail   = if edp.bypass() == 0
        {
          if edp.alarm() == 0
          {
            source
          }
          else
          {
            if edp.is_digital()
            {
               &edp.raw_data.to_string()
            }
            else
            {
              if edp.low() != 0 && edp.high() == 0 { "LOW" }
              else if edp.low() == 0 && edp.high() != 0 { "HIGH" }
              else { "ANALOG" }
            }
          }
        }
        else { "BYPASS" };

        let alarm: Alarm =
        [
          ("timestamp".to_owned(),  edp.seconds.to_string()),
          ("device".to_owned(),     edp.name.to_owned()),
          ("source".to_owned(),     source.to_owned()),
          ("severity".to_owned(),   severity.to_owned()),
          ("detail".to_owned(),     detail.to_owned()),
          ("message".to_owned(),    edp.text.to_owned()),
        ];

        match sndr.send(alarm).await
        {
          Ok(_) => (),
          Err(_) => return Ok(()),
        }

        rec = &rec[192..];
      }
    }
  }
}

async fn redis_task(mut rcvr: Receiver<Alarm>, redis_addr: String, redis_port: u16, stream_key: String) -> RedisResult<()>
{
  let uri = format!("redis://{redis_addr}:{redis_port}");

  let client = Client::open(uri)?;

  let config = ConnectionManagerConfig::new();

  let mut cxnmgr = ConnectionManager::new_lazy_with_config(client, config)?;

  println!("{}", format!("\nUsing Redis at address {redis_addr} on port {redis_port} to stream {stream_key}").white());

  loop
  {
    match rcvr.recv().await
    {
      Some(alarm) =>
      {
        let _: Result<(), _> = cxnmgr.xadd_maxlen(&stream_key, StreamMaxlen::Approx(9999), "*", &alarm).await?;
      },
      None => return Ok(()),
    }
  }
}

#[derive(Parser)]
struct Args
{
  /// Address of AEOLUS multicast
  #[arg(short, default_value_t = String::from("239.128.1.1"))]
  aeolus_multicast: String,

  /// Local listen port
  #[arg(short, default_value_t = 4357)]
  local_port: u16,

  /// Address of Redis server
  #[arg(short, default_value_t = String::from("127.0.0.1"))]
  redis_address: String,

  /// Port of Redis server
  #[arg(short='p', default_value_t = 6379)]
  redis_port: u16,

  /// Key name for Redis stream
  #[arg(short, default_value_t = String::from("acorn:alarms"))]
  stream_key: String,
}

#[tokio::main]
async fn main()
{
  let args = Args::parse();

  let (sndr, rcvr) = tokio::sync::mpsc::channel::<Alarm>(1000);

  let atask = tokio::spawn(aeolus_task(sndr, args.aeolus_multicast, args.local_port));

  let rtask = tokio::spawn(redis_task(rcvr, args.redis_address, args.redis_port, args.stream_key));

  let _ = tokio::join!(atask, rtask);
}
