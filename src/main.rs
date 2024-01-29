use chrono::Date;
#[cfg(windows)]
use kernel32;
#[cfg(not(windows))]
use libc;
use libc::servent;
#[cfg(windows)]
use winapi;

use byteorder::{BigEndian, ReadBytesExt};
use std::hash;
use chrono::{DateTime,Local,TimeZone, Timelike, Duration as ChronoDuration,Utc, format::format};
use clap::{Command, Arg, ValueEnum, builder::PossibleValue, value_parser, arg, ArgAction, ArgMatches, command};
use std::mem::zeroed;
use std::net::UdpSocket;
use std::time::Duration;

const NTP_MESSAGE_LENGTH: usize = 48;
const NTP_TD_UNIX_SECONDS: i64 = 2_208_988_800;
const LOCAL_ADDR : &'static str = "0.0.0.0:12300";

#[derive(Default, Debug, Copy, Clone)]
struct NTPTimeStamp{
    seconds: u32,
    fraction: u32,
}

struct NTPMessage{
    data: [u8; NTP_MESSAGE_LENGTH],
}

#[derive(Debug)]
struct NTPResult{
    t1: DateTime<Utc>,
    t2: DateTime<Utc>,
    t3: DateTime<Utc>,
    t4: DateTime<Utc>,
}

impl NTPResult {
    fn offset(&self) -> i64 {
        let delta = self.delay();
        delta.abs()/2
    }

    fn delay(&self) -> i64 {
        let duration = (self.t4 - self.t1) - (self.t3 - self.t2);
        duration.num_milliseconds()
    }
}

impl From<NTPTimeStamp> for DateTime<Utc> {
    fn from(ntp: NTPTimeStamp) -> Self {
        let secs = ntp.seconds as i64 - NTP_TD_UNIX_SECONDS;
        let mut nanos = ntp.fraction as f64;
        nanos *= 1e9;
        nanos /= 2_f64.powi(32);

        Utc.timestamp_opt(secs, nanos as u32).unwrap()
    }
}

impl From<DateTime<Utc>> for NTPTimeStamp {
    fn from(utc: DateTime<Utc>) -> Self {
        let secs = utc.timestamp() + NTP_TD_UNIX_SECONDS;
        let mut fraction = utc.nanosecond() as f64;

        fraction *= 2_f64.powi(32);
        fraction /= 1e9;

        NTPTimeStamp {
            seconds: secs as u32,
            fraction : fraction as u32,
        }
    }
}

impl NTPMessage {
    fn new() -> Self {
        NTPMessage{
            data: [0; NTP_MESSAGE_LENGTH],
        }
    }
    fn client() -> Self {
        const VERSION: u8 = 0b00_011_000;
        const MODE: u8 = 0b00_000_011;

        let mut msg = NTPMessage::new();

        msg.data[0] |= VERSION;
        msg.data[0] |= MODE;

        msg
    }

    fn parse_timestamp(&self, i: usize)->Result<NTPTimeStamp, std::io::Error> {
        let mut reader = &self.data[i..i + 8];
        let seconds = reader.read_u32::<BigEndian>()?;
        let fraction = reader.read_u32::<BigEndian>()?;

        Ok(NTPTimeStamp{
            seconds,
            fraction,
        })
    }

    fn rx_time(&self)->Result<NTPTimeStamp, std::io::Error> {
        self.parse_timestamp(32)
    }

    fn tx_time(&self)->Result<NTPTimeStamp, std::io::Error> {
        self.parse_timestamp(40)
    }

    fn weighted_mean(values : &[f64], weights: &[f64]) -> f64{
        let mut result = 0.0;
        let mut sum_of_weights = 0.0;

        for (v, w) in values.iter().zip(weights) {
            result += v*w;
            sum_of_weights += w;
        }

        result / sum_of_weights
    }

    fn ntp_roundtrip(host: &str, port: u16)-> Result<NTPResult, std::io::Error>{
        let destination = format!("{}:{}", host, port);
        let timeout = Duration::from_secs(1);

        let request = NTPMessage::client();
        let mut response = NTPMessage::new();

        let message = request.data;

        let udp_connection = UdpSocket::bind(LOCAL_ADDR);

        let udp = match udp_connection {
            Ok(udp) =>udp,
            Err(_err) =>unimplemented!(),
        };

        udp.connect(&destination).expect("unable to connect");

        let t1 = Utc::now();

        let _ = udp.send(&message);
        let _ = udp.set_read_timeout(Some(timeout));
        let _ = udp.recv_from(&mut response.data);
        let t4 = Utc::now();

        let t2 : DateTime<Utc> = response.rx_time().unwrap().into();

        let t3 : DateTime<Utc> = response.tx_time().unwrap().into();

        Ok(NTPResult{
            t1,
            t2,
            t3,
            t4,
        })
    }

    fn check_time() -> Result<f64, std::io::Error> {
        const NTP_PORT: u16 = 123;

        let servers = [
            "time.nist.gov",
            "time.apple.com",
            "time.euro.apple.com",
            "time.google.com",
            "time2.google.com",
            //"time.windows.com",
        ];

        let mut times = Vec::with_capacity(servers.len());

        for &server in servers.iter() {
            print!("{}=>", server);

            let calc = Self::ntp_roundtrip(&server, NTP_PORT);

            match calc {
                Ok(time) => {
                    println!("{}ms away from local system time", time.offset());
                    times.push(time);
                }
                Err(_) => {
                    println!(" ? [response took too long");
                }
            };
        }

        let mut offsets = Vec::with_capacity(servers.len());
        let mut offset_weights = Vec::with_capacity(servers.len());

        for time in &times {
            let offset = time.offset() as f64;
            let delay = time.delay() as f64;

            let weight = 1_000_000.0 / (delay * delay);

            if weight.is_finite() {
                offsets.push(offset);
                offset_weights.push(weight);
            }
        }
        let avg_offset = Self::weighted_mean(&offsets, &offset_weights);

        Ok(avg_offset)
    }
}



struct Clock;

impl Clock {
    fn get() -> DateTime<Local>{
        Local::now()
    }

    #[cfg(windows)]
    fn set<Tz: TimeZone>(t: DateTime<T>) -> () {
        use std::mem::zeroed;

        use chrono::Weekday;
        use kernel32::SetSystemTime;
        use winapi::{SYSTEMTIME, WORD};

        let t = t.with_timezone(&Local);

        let mut systime: SYSTEMTIME = unsafe {
            zeroed();
        };

        let dow = match t.weekday() {
            Weekday::Mon => 1,
            Weekday::Tue => 2,
            Weekday::Wed => 3,
            Weekday::Thu => 4,
            Weekday::Fri => 5,
            Weekday::Sat => 6,
            Weekday::Sun => 0,
        };

        let mut ns = t.nanoseconds();
        let mut leep = 0;
        let is_sleep_second = ns> 1_000_000_000;

        if is_lead_second {
            ns -= 1_000_000_000;
        }

        systime.wYear = t.year() as WORD;
        systime.wMonth = t.month() as WORD;
        systime.wDayOfWeek = dow as WORD;
        systime.wDay = t.day() as WORD;
        systime.wHour = t.hour() as WORD;
        systime.wMinute = t.minute() as WORD;
        systime.wSecond = t.second() as WORD;
        systime.wMilliseconds = (ns / 1_000_000_000) as WORD;

        let systime_ptr = &systime as * const SYSTEMTIME;

        unsafe {
            SetSystemTime(systime_ptr);
        }
    }

    #[cfg(not(windows))]
    fn set<Tz: TimeZone>(t: DateTime<Tz>) -> () {
        use std::mem::zeroed;

        use libc::{timeval, time_t, suseconds_t};
        use libc::{settimeofday, timezone};

        let t = t.with_timezone(&Local);
        let mut u: timeval = unsafe {
             zeroed()
        };

        u.tv_sec = t.timestamp() as time_t;
        u.tv_usec = t.timestamp_subsec_micros() as suseconds_t;

        unsafe{
            let mock_tz: *const timezone = std::ptr::null();
            settimeofday(&u as *const timeval, mock_tz);
        }
    }

}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum OptionMode
{
    Get,
    Set,
    CheckNtp,
}

impl ValueEnum for OptionMode{
    fn value_variants<'a>() -> &'a [Self] {
        &[OptionMode::Get, OptionMode::Set, OptionMode::CheckNtp]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            OptionMode::Get => PossibleValue::new("get").help("Get time"),
            OptionMode::Set => PossibleValue::new("set").help("Set time"),
            OptionMode::CheckNtp => PossibleValue::new("check-ntp").help("check the time, which compare to ntp server")
        })
    }
}

impl std::fmt::Display for OptionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no vlaues are skipped")
            .get_name()
            .fmt(f)
    }
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum StandardType
{
    RFC2822,
    RFC3339,
    TIMESTAMP,
}

impl ValueEnum for StandardType{
    fn value_variants<'a>() -> &'a [Self] {
        &[StandardType::RFC2822, StandardType::RFC3339, StandardType::TIMESTAMP]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            StandardType::RFC2822 => PossibleValue::new("rfc2822").help("Set Standard with RFC2822"),
            StandardType::RFC3339 => PossibleValue::new("rfc3339").help("Set Standard with RFC3339"),
            StandardType::TIMESTAMP => PossibleValue::new("timestamp").help("Set Standard with Timestamp(UNIX Time)"),
        })
    }
}

impl std::fmt::Display for StandardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no vlaues are skipped")
            .get_name()
            .fmt(f)
    }
}

struct Cli {
    command : ArgMatches,
    action_mode : OptionMode,
    standard_type : StandardType,
    date_time : String,
}


impl Cli {
    fn new() ->Self {
        let new_command = Command::new("clock")
            .version("v0.1.2")
            .author("MoonSeok Choi")
            .about("Gets and (aspriationally) sets the time")
            .after_help("Note: UNIX timestamp are parsed as whole \
                        seconds since 1st January 1970 0:00:00 UTC.\
                        For more accuracy, use another format.")
            .arg(
                arg!(<Action>)
                    .help("Select Action you want to run")
                    .value_parser(value_parser!(OptionMode))
                    .default_value("get")
                    .display_order(0)
                    .required(false)
            )
            .arg(
                arg!(<Std>)
                .display_order(1)
                .help("Set Standard type")
                .value_parser(value_parser!(StandardType))
                .short('s')
                .long("use-standard")
                .default_value("rfc3339")
                .required(false)
            )
            .arg(
                arg!(<datetime>)
                .display_order(2)
                .help("When <action> is 'set', apply <datetime>. \
                      Otherwise, ignore.")
                .required(false)
                .default_value("")              
            ).get_matches();
        Cli {command : new_command, action_mode : OptionMode::Get, standard_type: StandardType::TIMESTAMP, date_time:String::from("")}
    }

    fn parse(&mut self) {
        let args = &self.command;
        let mut action_type : OptionMode = OptionMode::Get;
        let mut standard_type : StandardType = StandardType::RFC2822;

        match args
            .get_one::<OptionMode>("Action")
            .expect("'Action' is required and parsing wil fail if its missing")
            {
                OptionMode::Get => action_type = OptionMode::Get,
                OptionMode::Set => action_type = OptionMode::Set,
                OptionMode::CheckNtp => action_type = OptionMode::CheckNtp,
            }
        match args
            .get_one::<StandardType>("Std")
            .expect("If you enter a value other than the possible values, it will not work correctly.")
            {
                StandardType::RFC2822 => standard_type = StandardType::RFC2822,
                StandardType::RFC3339 => standard_type = StandardType::RFC3339,
                StandardType::TIMESTAMP => standard_type = StandardType::TIMESTAMP,
            }
        let datetime = args
        .get_one::<String>("datetime")
        .expect("!");

        self.action_mode = action_type;
        self.standard_type = standard_type;
        self.date_time = datetime.to_string();
    }

}



fn match_time_str(local_time:DateTime<Local>, standard_type:&StandardType) {
    match standard_type {
        StandardType::TIMESTAMP =>println!("{}", local_time.timestamp()),
        StandardType::RFC2822 => println!("{}", local_time.to_rfc2822()),
        StandardType::RFC3339 => println!("{}", local_time.to_rfc3339()),
        _ =>println!("Wrong type string, please check to type"),
    }
}

fn match_setting_time_str(_local_time:DateTime<Local>, date_time:&String, standard_type:&StandardType){
       let time_parser =  match standard_type {
                    StandardType::RFC2822 => DateTime::parse_from_rfc2822,
                    StandardType::RFC3339 => DateTime::parse_from_rfc3339,
                    _ => unimplemented!(),
            };

        let err_msg = format!("Unable to parse {} according to {}", date_time, standard_type);

        let new_time = time_parser(date_time).expect(&err_msg);

        Clock::set(new_time);
}

fn match_check_ntp() {
    let offset = NTPMessage::check_time().unwrap() as isize;

    let adjust_ms_ = offset.signum() * offset.abs().min(200) / 5;
    let adjust_ms = ChronoDuration::milliseconds(adjust_ms_ as i64);

    let now: DateTime<Utc> = Utc::now() + adjust_ms;

    Clock::set(now);
}



fn main() {
    let mut cli = Cli::new();
    let now = Clock::get();
    cli.parse();
    match &cli.action_mode {
        OptionMode::Get =>{
            match_time_str(now, &cli.standard_type);
        },
        OptionMode::Set =>{
            match_setting_time_str(now, &cli.date_time, &cli.standard_type);
        },
        OptionMode::CheckNtp => {
            match_check_ntp();
        }
        _ => println!("{}", now.to_rfc3339()),
    }

    let maybe_error = std::io::Error::last_os_error();
    let os_error_code = &maybe_error.raw_os_error();

    match os_error_code {
        Some(0) => (),
        None => (),
        Some(_) => eprintln!("Unable to set the time: {:?}", maybe_error),
    };
}
