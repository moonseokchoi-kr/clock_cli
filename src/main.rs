use std::hash;

use chrono::{DateTime,Local,TimeZone, format::format};
use clap::{Command, Arg, ValueEnum, builder::PossibleValue, value_parser, arg, ArgAction, ArgMatches, command};

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
            leap += 1;
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
}

impl ValueEnum for OptionMode{
    fn value_variants<'a>() -> &'a [Self] {
        &[OptionMode::Get, OptionMode::Set]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            OptionMode::Get => PossibleValue::new("get").help("Get time"),
            OptionMode::Set => PossibleValue::new("set").help("Set time"),
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

fn match_setting_time_str(local_time:DateTime<Local>, date_time:&String, standard_type:&StandardType){
       let time_parser =  match standard_type {
                    StandardType::RFC2822 => DateTime::parse_from_rfc2822,
                    StandardType::RFC3339 => DateTime::parse_from_rfc3339,
                    _ => unreachable!(),
            };

        let err_msg = format!("Unable to parse {} according to {}", date_time, standard_type);

        let new_time = time_parser(date_time).expect(&err_msg);

        Clock::set(new_time);

        let maybe_error = std::io::Error::last_os_error();
        let os_error_code = &maybe_error.raw_os_error();

        match os_error_code {
            Some(0) => (),
            None => (),
            Some(_) => eprintln!("Unable to set the time: {:?}", maybe_error),
        };
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
        _ => println!("{}", now.to_rfc3339()),
    }
}
