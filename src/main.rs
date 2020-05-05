extern crate crossbeam_channel;

use chrono::prelude::*;
use clap::{self, Arg};
use crossbeam_channel::{after, bounded, select, tick, unbounded, Receiver, Sender};
use parse_duration::parse;
use rodio::{self, Sink, Source};
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::thread;
use std::time;

fn get_time() -> String {
    let local = Local::now();
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

fn run_mp3(fpath: &str, repeat: u8) {
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

fn clear() {
    std::process::Command::new("clear")
        .status()
        .unwrap()
        .success();
    println!("\n{}\n", "==== START ALARM ====");
}

fn main() {
    #[allow(unused_variables)]
    let matches = clap::App::new("Alarm")
        .arg(
            Arg::with_name("time")
                .short("t")
                .long("time")
                .value_name("TIME")
                .help("Duration in which the alarm will be trigerred")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("reminder")
                .short("r")
                .long("reminder")
                .value_name("REMINDER")
                .help("Reminder of alarm")
                .takes_value(true),
        )
        .get_matches();

    let alarm_time = matches.value_of("time").unwrap_or("14 minutes 20 seconds");
    let alarm_time = match parse(alarm_time) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Can't parse '{}'", alarm_time);
            std::process::exit(1);
        }
    };

    let reminder_time = matches.value_of("reminder").unwrap_or("1 minutes");
    let reminder_time = match parse(reminder_time) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Can't parse '{}'", reminder_time);
            std::process::exit(1);
        }
    };

    let (tx_alarm_reset, rx_alarm_reset) = unbounded();

    let alarm_channel = get_alarm(alarm_time, reminder_time, rx_alarm_reset);

    let keyboard_channel = get_keyboard_channel();

    clear();
    loop {
        select! {
            recv(alarm_channel.reminder) -> date => {
                println!("{}", date.unwrap());
            },
            recv(alarm_channel.alarm) -> date => {
                println!("{} [RUN ALARAM]", date.unwrap());
                thread::spawn(|| {
                    run_mp3("1.mp3", 3);
                });
            },
            recv(keyboard_channel) -> s => {
                clear();
                if s.unwrap().trim() == "r" {
                    tx_alarm_reset.send(true).unwrap();
                }
            }
        }
    }
}
