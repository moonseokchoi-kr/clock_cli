use chrono::{DateTime};
use chrono::{Local};
use clap::{Args, Subcommand, Parser};

struct Clock;

impl Clock {
    fn get() -> DateTime<Local>{
        Local::now()
    }

    fn set() -> ! {
        unimplemented!()
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command : Option<Commands>
}

#[derive(Subcommand)]
enum Commands {
    Get(GetArgs),
    Set {
        #[arg(short = 's' ,long, value_name = "std")]
        use_standard: String,
    }
}

#[derive(Args)]
struct GetArgs{
    #[arg(short = 's',long, value_name = "std", default_value = "rfc3339")]
    use_standard: Option<String>
}

#[derive(Args)]
struct SetArgs{
    #[arg(short = 's' ,long, value_name = "std", default_value = "rfc3339")]
    use_standard: Option<String>,
}

fn match_time_str(local_time:DateTime<Local>, type_string:&Option<String>) {

    match type_string {
        Some(type_string) => {
            let std = type_string;
            match std.as_str() {
                "timestamp" =>println!("{}", local_time.timestamp()),
                "rfc2822" => println!("{}", local_time.to_rfc2822()),
                "rfc3339" => println!("{}", local_time.to_rfc3339()),
                _ => {
                    if type_string.is_empty(){
                        println!("{}", local_time.to_rfc3339());
                    }else{
                        println!("Wrong type string, please check to type");
                    }
                }
            }
        }
        None => println!("Undefined Error!!"),
    }
}

fn main() {
    let cli = Cli::parse();
    let now = Clock::get();
    match &cli.command {
        Some(Commands::Get(get_args)) =>{
            match_time_str(now, &get_args.use_standard);
        },
        Some(Commands::Set { use_standard }) =>{
            Clock::set();
        },
        None => println!("{}", now.to_rfc3339()),
    }
}
