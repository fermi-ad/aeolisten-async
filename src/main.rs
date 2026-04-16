mod aeolus; mod edp;

use crate::aeolus::{aeolus_task, Alarm};

use clap::Parser;
use colored::Colorize;
use redis::{AsyncCommands, Client};
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::streams::StreamMaxlen;
use std::error::Error;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;

async fn redis_task(mut rcvr: Receiver<Alarm>, redis_addr: String, redis_port: u16, stream_key: String) -> Result<(), Box<dyn Error>> {
  //  create a "self-healing" connection to redis
  let uri = format!("redis://{redis_addr}:{redis_port}");

  let client = Client::open(uri)?;

  let config = ConnectionManagerConfig::new();

  let mut cxnmgr = ConnectionManager::new_lazy_with_config(client, config)?;

  println!("{}", format!("\nTrying Redis at address {redis_addr} on port {redis_port} to stream {stream_key}").white());

  //  wait for alarm messages from aeolus task and push them to redis
  loop {
    let alarm = rcvr.recv().await;
    if alarm.is_some() {
      let _: Result<(), _> = cxnmgr.xadd_maxlen(&stream_key, StreamMaxlen::Approx(9999), "*", &alarm.unwrap()).await;
    }
  }
}

#[derive(Parser)]
struct Args {
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
async fn main() -> Result<(), Box<dyn Error>> {
  let args = Args::parse();

  //  create a message queue for aeolus task to pass alarms to redis task
  let (sndr, rcvr) = mpsc::channel::<Alarm>(1000);

  //  start aeolus task with sender for message queue
  //  start redis task with receiver for message queue
  let (ajoin, rjoin) = tokio::join!(
    aeolus_task(sndr, args.aeolus_multicast, args.local_port),
    redis_task(rcvr, args.redis_address, args.redis_port, args.stream_key),
  );
  match (ajoin, rjoin) {
    (Ok(_), Ok(_)) => Ok(()),
    (Err(ae), Ok(_)) => { println!("aeolus join: {ae}"); Err(ae) },
    (Ok(_), Err(re)) => { println!("redis join: {re}"); Err(re) },
    (Err(ae), Err(re)) => {
      println!("aeolus join: {ae}");
      println!("redis join: {re}");
      Err(ae)
    },
  }
}
