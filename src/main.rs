extern crate crossbeam_channel;

use ansi_term::Color;
use crossbeam_channel::{after, bounded, select, tick, unbounded, Receiver, Sender};
use parse_duration::parse as parse_duration;
use rodio::{self, Sink, Source};
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;
use std::thread;
use std::time::{self, Duration};
use structopt::StructOpt;

fn get_time() -> String {
    let local = chrono::Local::now();
    local.format("%Y-%m-%d %H:%M:%S").to_string()
}

struct AlarmChannel {
    alarm: Receiver<String>,
    reminder: Receiver<String>,
}

fn run_reminder(
    interval: time::Duration,
    reminder_channel: Sender<String>,
    quit_channel: Receiver<bool>,
) {
    thread::spawn(move || {
        let ticker = tick(interval);
        let mut i = 0;
        loop {
            i += 1;
            select! {
                recv(ticker) -> x => {
                    if let Err(a) = reminder_channel.send(format!("{} [{}]", get_time(), i)) {
                        eprintln!("Failed to send to channel {:?} {:?}", a, x);
                    }
                },
                recv(quit_channel) -> _ => {
                    break;
                }
            }
        }
    });
}

fn run_mp3(fpath: PathBuf, repeat: u8) {
    let f = File::open(fpath).unwrap();
    let device = rodio::default_output_device().unwrap();
    let sink = Sink::new(&device);

    let source = rodio::Decoder::new(BufReader::new(f)).unwrap().buffered();

    (0..repeat).for_each(|_| sink.append(source.clone()));

    sink.play();
    sink.sleep_until_end();
}

fn get_alarm(
    interval: time::Duration,
    interval_reminder: time::Duration,
    restart_channel: Receiver<bool>,
) -> AlarmChannel {
    let (tx_alarm, rx_alarm) = unbounded();
    let (tx_reminder_quit, rx_reminder_quit) = bounded(0);
    let (tx_reminder_tick, rx_reminder_tick) = unbounded();

    thread::spawn(move || loop {
        run_reminder(
            interval_reminder,
            tx_reminder_tick.clone(),
            rx_reminder_quit.clone(),
        );

        select! {
            recv(after(interval)) -> x => {
                if let Err(a) =  tx_alarm.send(get_time()) {
                    eprintln!("Failed to send to channel {:?} {:?}", a, x);
                }
            },
        }
        restart_channel.recv().unwrap();
        tx_reminder_quit.send(true).unwrap();
    });

    AlarmChannel {
        alarm: rx_alarm,
        reminder: rx_reminder_tick,
    }
}

fn get_keyboard_channel() -> Receiver<String> {
    let (tx, rx) = unbounded();

    io::stdin().lock();
    thread::spawn(move || loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        tx.send(buffer).unwrap();
    });
    rx
}

fn duration_to_display(d: Duration) -> String {
    const PER_MINUTES: u64 = 60;
    const PER_HOUR: u64 = PER_MINUTES * 60;

    let secs = d.as_secs();

    let h = secs / PER_HOUR;
    let m = (secs % PER_HOUR) / PER_MINUTES;
    let s = secs % PER_MINUTES;

    let mut res = String::new();

    if h > 0 {
        res += &format!("{} hours   ", h);
    }
    if m > 0 || h > 0 {
        res += &format!("{:>2} minutes   ", m);
    }
    if s > 0 || m > 0 || h > 0 {
        res += &format!("{:>2} seconds   ", s);
    }
    res
}

fn reset(opt: &Opt) {
    std::process::Command::new("clear")
        .status()
        .unwrap()
        .success();
    println!(
        "\n{}\n",
        Color::Cyan
            .bold()
            .paint("=========== START ALARM ===========")
    );
    println!("File     : {}\n", opt.file.to_str().unwrap());
    println!("Alarm    : {}", duration_to_display(opt.time));
    println!("Reminder : {}\n", duration_to_display(opt.reminder));
}

fn parse_mp3_path<P>(path: P) -> Result<PathBuf, String>
where
    P: AsRef<str>,
{
    let path = PathBuf::from(
        shellexpand::full(&path)
            .map_err(|_| "Invalid path")?
            .into_owned(),
    );

    let mut file = File::open(&path).map_err(|e| e.to_string())?;

    let mut buf = [0; 3];
    file.read_exact(&mut buf).map_err(|e| e.to_string())?;

    // if buf.as_ref() != b"ID3" {
    //     return Err("Not a valid MP3 file".to_owned());
    // }

    Ok(path)
}

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(short, long, parse(try_from_str = parse_duration))]
    time: Duration,

    #[structopt(short, long, parse(try_from_str = parse_duration))]
    reminder: Duration,

    #[structopt(short, long, parse(try_from_str = parse_mp3_path))]
    file: PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt.file);

    let (tx_alarm_reset, rx_alarm_reset) = unbounded();

    let alarm_channel = get_alarm(opt.time, opt.reminder, rx_alarm_reset);

    let keyboard_channel = get_keyboard_channel();

    reset(&opt);
    loop {
        select! {
            recv(alarm_channel.reminder) -> date => {
                println!("{}", date.unwrap());
            },
            recv(alarm_channel.alarm) -> date => {
                println!("{}", Color::Blue.bold().paint(format!("{} [RUN ALARAM]", date.unwrap())));
                let file = opt.file.clone();
                thread::spawn(|| {
                    run_mp3(file, 2);
                });
            },
            recv(keyboard_channel) -> s => {
                if s.unwrap().trim() == "r" {
                    reset(&opt);
                    tx_alarm_reset.send(true).unwrap();
                }
            }
        }
    }
}
