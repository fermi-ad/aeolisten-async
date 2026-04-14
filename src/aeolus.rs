use byteorder::{BigEndian as BE, ReadBytesExt};
use colored::Colorize;
use socket2::{Domain, Protocol, Socket, Type};
use std::error::Error;
use std::io::Cursor;
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;

use crate::edp::EDP;

pub type Alarm = [(String, String); 6];

pub async fn aeolus_task(sndr: Sender<Alarm>, mcast_addr: String, listen_port: u16) -> Result<(), Box<dyn Error>>
{
  //  create listener socket for aeolus multicast
  let sock2 = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

  let _ = sock2.set_nonblocking(true);    //  socket should be nonblocking for tokio
  let _ = sock2.set_reuse_address(true);  //  and reusable address for multicast

  let addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, listen_port);

  let _ = sock2.bind(&addr.into());

  let group: Ipv4Addr = mcast_addr.parse()?;

  let _ = sock2.join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED);

  let sock = UdpSocket::from_std(sock2.into())?;

  println!("{}", format!("\nListening on port {listen_port} to multicast from address {mcast_addr}").white());

  let mut buf = [0u8; 9999];  //  big reusable receive buffer

  loop
  {
    //  wait for a datagram to arrive
    let (len, _) = sock.recv_from(&mut buf).await?;

    if len == buf.len() { println!("{}", "\nMax data received".bright_red()); }

    println!("{}", format!("\nReceived {len} bytes").white());

    if len < 33
    {
      println!("{}", "\nPacket too small".bright_red());
      continue;
    }

    let ip_ver    = buf[ 0] as f32 + buf[ 1] as f32 / 10.0;
    let mc_ver    = buf[20] as f32 + buf[21] as f32 / 10.0;
    let typecode  = buf[32];

    let mut cur = Cursor::new(&buf[..]);

    cur.set_position(24);
    let seq_num = cur.read_u32::<BE>()?;

    match typecode  //  print colored output for different message types
    {
      33 =>   //  Heartbeat
      {
        if len != 60
        {
          println!("{}", "HB packet length is not 60".bright_red());
          continue;
        }

        cur.set_position(32+4);

        let seconds    = cur.read_u32::<BE>()?;
        let edm_seq    = cur.read_u32::<BE>()?;
        let evt_seq    = cur.read_u32::<BE>()?;
        let evt_num    = cur.read_u32::<BE>()?;
        let hb_seq     = cur.read_u32::<BE>()?;
        let count      = cur.read_u32::<BE>()?;

        println!("{}", format!("HB  ip_ver: {ip_ver:.1}   mc_ver: {mc_ver:.1}   seq_num: {seq_num}   typecode: {typecode}   seconds: {seconds}").green());
        println!("{}", format!("    edm_seq: {edm_seq}    evt_seq: {evt_seq}    evt_num: {evt_num}   hb_seq: {hb_seq:4}   count: {count:3}").green());
      },
      3 | 10 | 86 | 87 | 90 =>   //  Multiclear/Appclear
      {
        if len != 104
        {
          println!("{}", "MCLR packet length is not 104".bright_red());
          continue;
        }

        cur.set_position(32+64);

        let daemon_id = cur.read_u32::<BE>()?;
        let edm_seq   = cur.read_u32::<BE>()?;

        println!("{}", format!("MCLR  ip_ver: {:.1}   mc_ver: {:.1}   seq_num: {}   typecode: {}   daemon_id: {}   edm_seq: {}",
                                      ip_ver,         mc_ver,         seq_num,      typecode,      daemon_id,      edm_seq).magenta());
      },
      0 | 1 | 78 =>   //  Event Display Message (has Event Display Packets)
      {
        cur.set_position(32+1);

        let count     = cur.read_u8()?;
        let version   = cur.read_u8()?;
        let edm_seq  = cur.read_u32::<BE>()?;

        let count_by_len = (len - 40) / 192;

        if count as usize != count_by_len { println!("{}", format!("EDM packet can hold {count_by_len} but count is {count}").bright_red()); }

        println!("{}", format!("EDM  ip_ver: {:.1}   mc_ver: {:.1}   seq_num: {}   typecode: {}   count: {}   version: {}   edm_seq: {}",
                                     ip_ver,         mc_ver,         seq_num,      typecode,      count,      version,      edm_seq).green());

        cur.set_position(32+8);  //  advance to first edp

        for _ in 0..std::cmp::min(count as usize, count_by_len)
        {
          let edp = EDP::new(&mut cur)?;

          println!();
          edp.colorprint();   //  EDP knows how to print itself

          //  build alarm entry for redis stream
          let source = if edp.is_digital() { "DIGITAL" } else { "ANALOG"};

          let severity = if edp.alarm()
          {
            if edp.priority < 10 { "MINOR" } else { "MAJOR" }
          }
          else { "NO_ALARM" };

          let detail = match (edp.bypass(), edp.alarm(), edp.is_digital(), edp.high(), edp.low())
          {
            (true,  _,     _,     _,     _    ) => "BYPASS",
            (false, true,  true,  _,     _    ) => &edp.raw_data.to_string(),
            (false, true,  false, false, true ) => "LOW",
            (false, true,  false, true,  false) => "HIGH",
            (false, true,  false, _,     _    ) => "ANALOG",
            (false, false, _,     _,     _    ) => source
          };

          let alarm: Alarm =  //  this is the redis stream entry
          [
            ("timestamp".to_owned(),  edp.seconds.to_string()),
            ("device".to_owned(),     edp.name.to_owned()),
            ("source".to_owned(),     source.to_owned()),
            ("severity".to_owned(),   severity.to_owned()),
            ("detail".to_owned(),     detail.to_owned()),
            ("message".to_owned(),    edp.text.to_owned()),
          ];

          //  send it to redis task via message queue
          match sndr.send(alarm).await
          {
            Ok(_) => (),
            Err(_) => return Ok(()),
          }
        }
      },
      76 =>
      {
        println!("{}", format!("RSND  ip_ver: {ip_ver:.1}   mc_ver: {mc_ver:.1}   seq_num: {seq_num}   typecode: {typecode}").magenta());
      },
      _ =>
      {
        println!("{}", format!("UNK  ip_ver: {ip_ver:.1}   mc_ver: {mc_ver:.1}   seq_num: {seq_num}   typecode: {typecode}").bright_red());
      }
    }
  }
}