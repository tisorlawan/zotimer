extern crate crossbeam_channel;

use chrono::prelude::*;
use crossbeam_channel::{after, bounded, select, tick, unbounded, Receiver, Sender};
use std::io;
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
                    if let Err(a) =  reminder_channel.send(format!("{} - {}", i, get_time())) {
                        eprintln!("Failed to send to channel {:?} {:?}", a, x);
                    }
                },
                recv(quit_channel) -> _ => {
                    break
                }
            }
        }
    });
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
        reminder: rx_reminder_tick.clone(),
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

fn main() {
    let (alarm_reset_tx, alarm_reset_rx) = unbounded();

    let alarm_channel = get_alarm(
        std::time::Duration::from_secs(10),
        std::time::Duration::from_secs(2),
        alarm_reset_rx.clone(),
    );

    let keyboard_channel = get_keyboard_channel();

    loop {
        select! {
            recv(alarm_channel.reminder) -> date => {
                println!("{}", date.unwrap());
            },
            recv(alarm_channel.alarm) -> date => {
                println!("ALARM {}", date.unwrap());
            },
            recv(keyboard_channel) -> s => {
                println!("KEY {}", s.unwrap());
                alarm_reset_tx.send(true).unwrap();
            }
        }
    }
}
