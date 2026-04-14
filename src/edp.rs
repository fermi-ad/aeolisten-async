use byteorder::{BigEndian as BE, ReadBytesExt};
use colored::Colorize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{Cursor, Read};

//  wrapper+trait to output bool as T/F
struct TF(bool);

impl Display for TF
{
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result
  {
    f.pad(if self.0 { "T" } else { "F" })
  }
}

//  class that can decode an EDP from bytes and print it with colors
pub struct EDP
{
  pub typecode: u8,     pub priority: u8,       pub trunk: u8,        pub node: u8,
  pub ssn: u8,          pub bs: u8,             pub erp_type: u8,

  pub dig_edp: bool,    pub broken: bool,

  pub unused: u8,

  pub status: u16,      pub handler: u16,       pub alarm_list: i16,

  pub dev_index: u32,   pub dev_class: u32,     pub dev_type: u32,    pub seconds: u32,
  pub seq_num: u32,     pub sound_id: u32,      pub speech_id: u32,   pub raw_data: u32,

  pub name: String,     pub full_name: String,  pub text: String,
}

impl EDP
{
  pub fn new(cur: &mut Cursor<&[u8]>) -> Result<Self, Box<dyn Error>>
  {
    Ok
    (
      Self
      {
        typecode: cur.read_u8()?,   priority: cur.read_u8()?,   trunk: cur.read_u8()?,      node: cur.read_u8()?,
        ssn: cur.read_u8()?,        bs: cur.read_u8()?,         erp_type: cur.read_u8()?,

        dig_edp: cur.read_u8()? != 0,   broken: cur.read_u8()? != 0,

        unused: cur.read_u8()?,

        status: cur.read_u16::<BE>()?,  handler: cur.read_u16::<BE>()?,   alarm_list: cur.read_i16::<BE>()?,

        dev_index: cur.read_u32::<BE>()?,   dev_class: cur.read_u32::<BE>()?,
        dev_type: cur.read_u32::<BE>()?,    seconds: cur.read_u32::<BE>()?,
        seq_num: cur.read_u32::<BE>()?,     sound_id: cur.read_u32::<BE>()?,
        speech_id: cur.read_u32::<BE>()?,   raw_data: cur.read_u32::<BE>()?,

        name:
        {
          let mut buf = vec![0u8; 16];
          cur.read_exact(&mut buf)?;
          str::from_utf8(&buf)?.trim_matches('\0').trim().to_string()
        },
        full_name:
        {
          let mut buf = vec![0u8; 64];
          cur.read_exact(&mut buf)?;
          str::from_utf8(&buf)?.trim_matches('\0').trim().to_string()
        },
        text:
        {
          let mut buf = vec![0u8; 64];
          cur.read_exact(&mut buf)?;
          str::from_utf8(&buf)?.trim_matches('\0').trim().to_string()
        },
      }
    )
  }

  pub fn bypass(&self)   -> bool { (self.status & (1 << 0)) == 0 }  //  reverse logic
  pub fn alarm(&self)    -> bool { (self.status & (1 << 1)) != 0 }
  pub fn trigger(&self)  -> bool { (self.status & (1 << 2)) != 0 }
  pub fn inhibit(&self)  -> bool { (self.status & (1 << 3)) != 0 }
  pub fn reserved(&self) -> bool { (self.status & (1 << 4)) != 0 }

  pub fn q_code(&self) -> u8 { ((self.status >> 5) & 3) as u8 }   //  bits 5-6

  pub fn dig_st(&self) -> bool { (self.status & (1 << 7)) != 0 }

  pub fn k_code(&self) -> u8 { ((self.status >> 8) & 7) as u8 }   //  bits 8-10

  pub fn low(&self)       -> bool { (self.status & (1 << 11)) != 0 }
  pub fn high(&self)      -> bool { (self.status & (1 << 12)) != 0 }
  pub fn exception(&self) -> bool { (self.status & (1 << 13)) != 0 }
  pub fn logging(&self)   -> bool { (self.status & (1 << 14)) != 0 }
  pub fn display(&self)   -> bool { (self.status & (1 << 15)) != 0 }

  pub fn is_digital(&self)  -> bool { self.dig_edp || self.dig_st() }   //  does either bit claim digital
  pub fn is_mismatch(&self) -> bool { self.dig_edp != self.dig_st() }   //  do both digital bits agree
  pub fn is_low_high(&self) -> bool { self.low() && self.high() }       //  are both low and high set

  pub fn colorprint(&self)
  {
    let line1 =
    {
      let txt = format!("name: {}   seconds: {}   seq_num: {}   full_name: {}   text: {}",
                                 self.name, self.seconds, self.seq_num, self.full_name, self.text);
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };

    //  typecode: 123   priority: 123   trunk: 12345   node: 123456789   ssn: 12345678   dev_type: 1234567
    //  broken: 12345   unused: 12345   handler: 123   alarm_list: 123   erp_type: 123   dev_index: 123456
    //  dev_class: 12   bs: 123456789   sound_id: 12   speech_id: 1234   dig_edp: 1234   raw_data: 0x12345678

    let line2 =
    {
      let txt = format!("typecode: {:3}   priority: {:3}   trunk: {:5}   node: {:9}   ssn: {:8}   dev_type: {:7}",
                                 self.typecode,   self.priority,   self.trunk,   self.node,   self.ssn,   self.dev_type);
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line3 =
    {
      let txt = format!("broken: {:>5}   unused: {:5}   handler: {:3}   alarm_list: {:3}   erp_type: {:3}   dev_index: {:6}",
                                TF(self.broken), self.unused,  self.handler,   self.alarm_list,   self.erp_type,   self.dev_index);
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line4a =
    {
      let txt = format!("dev_class: {:2}   bs: {:9}   sound_id: {:2}   speech_id: {:4}",
                                 self.dev_class,   self.bs,   self.sound_id,   self.speech_id);
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line4b =
    {
      let txt = format!("   dig_edp: {:>4}", TF(self.dig_edp));
      if self.is_mismatch() { txt.bright_red() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line4c =
    {
      let txt = format!("   raw_data: {:#7x}", self.raw_data);
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };

    //  status: 0x1234   bypass: 12   alarm: 1   trigger: 1   inhibit: 123   reserved: 1   q_code: 12
    //  dig_st:   1234   k_code: 12   low: 123   high: 1234   exception: 1   logging: 12   display: 1

    let line5a =
    {
      let txt = format!("status: {:#06x}   ", self.status);
      if self.status == 0 { txt.bright_red() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line5b =
    {
      let txt = format!("bypass: {:>2}   ", TF(self.bypass()));
      if self.bypass() { txt.bright_blue() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line5c =
    {
      let txt = format!("alarm: {:1}   ", TF(self.alarm()));
      if self.alarm() { txt.magenta() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line5d =
    {
      let txt = format!("trigger: {:1}   inhibit: {:>3}   reserved: {:1}   q_code: {:2}",
                        TF(self.trigger()), TF(self.inhibit()), TF(self.reserved()), self.q_code());
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line6a =
    {
      let txt = format!("dig_st: {:>6}   ", TF(self.dig_st()));
      if self.is_mismatch() { txt.bright_red() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line6b =
    {
      let txt = format!("k_code: {:2}   ", self.k_code());
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line6c =
    {
      let txt = format!("low: {:>3}   ", TF(self.low()));
      match (self.is_low_high(), self.low(), self.is_digital())
      {
        (true, _, _) => txt.bright_red(),
        (false, true, _) => txt.magenta(),
        (false, false, true) => txt.cyan(),
        (false, false, false) => txt.yellow(),
      }
    };
    let line6d =
    {
      let txt = format!("high: {:>4}   ", TF(self.high()));
      match (self.is_low_high(), self.high(), self.is_digital())
      {
        (true, _, _) => txt.bright_red(),
        (false, true, _) => txt.magenta(),
        (false, false, true) => txt.cyan(),
        (false, false, false) => txt.yellow(),
      }
    };
    let line6e =
    {
      let txt = format!("exception: {:1}   logging: {:>2}   display: {:1}",
                                 TF(self.exception()), TF(self.logging()), TF(self.display()));
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };

    println!("{line1}");
    println!("{line2}");
    println!("{line3}");
    println!("{line4a}{line4b}{line4c}");
    println!("{line5a}{line5b}{line5c}{line5d}");
    println!("{line6a}{line6b}{line6c}{line6d}{line6e}");
  }
}
