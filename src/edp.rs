use colored::Colorize;

pub fn be_u32(data: &[u8], idx:usize) -> u32
{
  u32::from_be_bytes(data[idx..idx+4].try_into().unwrap())
}

pub fn be_u16(data: &[u8], idx:usize) -> u16
{
  u16::from_be_bytes(data[idx..idx+2].try_into().unwrap())
}

pub fn be_i16(data: &[u8], idx:usize) -> i16
{
  i16::from_be_bytes(data[idx..idx+2].try_into().unwrap())
}
pub struct EDP
{
  pub typecode: u8,   pub priority: u8,   pub trunk: u8,    pub node: u8,     pub ssn: u8,
  pub bs: u8,         pub erp_type: u8,   pub dig_edp: u8,  pub broken: u8,   pub unused: u8,

  pub status: u16,    pub handler: u16,   pub alarm_list: i16,

  pub dev_index: u32,   pub dev_class: u32,   pub dev_type: u32,    pub seconds: u32,
  pub seq_num: u32,     pub sound_id: u32,    pub speech_id: u32,   pub raw_data: u32,

  pub name: String,   pub full_name: String,  pub text: String,
}

impl EDP
{
  pub fn new(buf: &[u8]) -> Self
  {
    Self
    {
      typecode: buf[0],   priority: buf[1],   trunk: buf[2],    node: buf[3],     ssn: buf[4],
      bs: buf[5],         erp_type: buf[6],   dig_edp: buf[7],  broken: buf[8],   unused: buf[9],

      status: be_u16(buf, 10),  handler: be_u16(buf, 12), alarm_list: be_i16(buf, 14),

      dev_index: be_u32(buf, 16),   dev_class: be_u32(buf, 20),
      dev_type: be_u32(buf, 24),    seconds: be_u32(buf, 28),
      seq_num: be_u32(buf, 32),     sound_id: be_u32(buf, 36),
      speech_id: be_u32(buf, 40),   raw_data: be_u32(buf, 44),

      name: str::from_utf8(&buf[48..64]).unwrap().trim_end_matches('\0').trim_end().to_string(),
      full_name: str::from_utf8(&buf[64..128]).unwrap().trim_end_matches('\0').trim_end().to_string(),
      text: str::from_utf8(&buf[128..192]).unwrap().trim_end_matches('\0').trim_end().to_string(),
    }
  }
  pub fn bypass(&self) -> u8    { ((self.status & 1) == 0)  as u8 }
  pub fn alarm(&self) -> u8     { ((self.status >>  1) & 1) as u8 }
  pub fn trigger(&self) -> u8   { ((self.status >>  2) & 1) as u8 }
  pub fn inhibit(&self) -> u8   { ((self.status >>  3) & 1) as u8 }
  pub fn reserved(&self) -> u8  { ((self.status >>  4) & 1) as u8 }
  pub fn q_code(&self) -> u8    { ((self.status >>  5) & 3) as u8 }
  pub fn dig_st(&self) -> u8    { ((self.status >>  7) & 1) as u8 }
  pub fn k_code(&self) -> u8    { ((self.status >>  8) & 7) as u8 }
  pub fn low(&self) -> u8       { ((self.status >> 11) & 1) as u8 }
  pub fn high(&self) -> u8      { ((self.status >> 12) & 1) as u8 }
  pub fn exception(&self) -> u8 { ((self.status >> 13) & 1) as u8 }
  pub fn logging(&self) -> u8   { ((self.status >> 14) & 1) as u8 }
  pub fn display(&self) -> u8   { ((self.status >> 15) & 1) as u8 }

  pub fn is_digital(&self) -> bool  { self.dig_edp != 0 || self.dig_st() != 0 }
  pub fn is_mismatch(&self) -> bool { self.dig_edp != self.dig_st() }
  pub fn is_low_high(&self) -> bool { self.low() != 0 && self.high() != 0 }

  pub fn println(&self)
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
      let txt = format!("broken: {:5}   unused: {:5}   handler: {:3}   alarm_list: {:3}   erp_type: {:3}   dev_index: {:6}",
                                 self.broken,   self.unused,   self.handler,   self.alarm_list,   self.erp_type,   self.dev_index);
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
      let txt = format!("   dig_edp: {:4}", self.dig_edp);
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
      let txt = format!("bypass: {:2}   ", self.bypass());
      if self.bypass() != 0 { txt.bright_blue() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line5c =
    {
      let txt = format!("alarm: {:1}   ", self.alarm());
      if self.alarm() != 0 { txt.magenta() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line5d =
    {
      let txt = format!("trigger: {:1}   inhibit: {:3}   reserved: {:1}   q_code: {:2}",
                                 self.trigger(), self.inhibit(), self.reserved(), self.q_code());
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line6a =
    {
      let txt = format!("dig_st: {:6}   ", self.dig_st());
      if self.is_mismatch() { txt.bright_red() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line6b =
    {
      let txt = format!("k_code: {:2}   ", self.k_code());
      if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line6c =
    {
      let txt = format!("low: {:3}   ", self.low());
      if self.is_low_high() { txt.bright_red() } else if self.low() != 0 { txt.magenta() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line6d =
    {
      let txt = format!("high: {:4}   ", self.high());
      if self.is_low_high() { txt.bright_red() } else if self.high() != 0 { txt.magenta() } else if self.is_digital() { txt.cyan() } else { txt.yellow() }
    };
    let line6e =
    {
      let txt = format!("exception: {:1}   logging: {:2}   display: {:1}",
                                 self.exception(), self.logging(), self.display());
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
