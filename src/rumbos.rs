//! What the rumbos a window holds use of the machine. A rumbo is one
//! session and a process of its own, with its provider and extensions under
//! it, so the sessions a window has open cost what their rumbos use
//! together. Each rumbo says its own processes, with `processes`; this
//! counts them, each process once, and writes the figures.

use std::collections::{HashMap, HashSet};

use crate::protocol::ProcessUsage;

/// How a session a rumbo is held for stands in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Shown,
    Working,
    Open,
}

impl Standing {
    pub fn said(self) -> &'static str {
        match self {
            Standing::Shown => "shown",
            Standing::Working => "working",
            Standing::Open => "open",
        }
    }
}

/// A rumbo the window holds: the session it is, by its title, how it
/// stands, and its processes as it said them last, or why it did not; none
/// until it answers.
#[derive(Debug, Clone)]
pub struct Rumbo {
    pub title: String,
    pub standing: Standing,
    pub processes: Option<Result<Vec<ProcessUsage>, String>>,
}

/// What some processes use together.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Used {
    pub processes: usize,
    pub memory: i64,
    pub cpu: f64,
}

impl Used {
    pub fn add(&mut self, other: Used) {
        self.processes += other.processes;
        self.memory += other.memory;
        self.cpu += other.cpu;
    }
}

/// What each rumbo uses, in the order given, with each process counted
/// once. A rumbo that started the window lists the window under it, and so
/// the rumbos the window started and what those started: they are taken out
/// of its count, since each of them says its own.
pub fn used(rumbos: &[&[ProcessUsage]]) -> Vec<Used> {
    // A rumbo lists itself first.
    let hosts: Vec<Option<i64>> = rumbos
        .iter()
        .map(|listed| listed.first().map(|process| process.pid))
        .collect();

    rumbos
        .iter()
        .zip(&hosts)
        .map(|(listed, own)| {
            let others: HashSet<i64> = hosts
                .iter()
                .flatten()
                .copied()
                .filter(|pid| Some(*pid) != *own)
                .collect();

            let parents: HashMap<i64, i64> = listed
                .iter()
                .map(|process| (process.pid, process.parent))
                .collect();

            // Whether a process is another rumbo, or was started by one,
            // however far above it.
            let under_another = |pid: i64| {
                let mut at = pid;

                for _ in 0..=parents.len() {
                    if others.contains(&at) {
                        return true;
                    }

                    match parents.get(&at) {
                        Some(parent) => at = *parent,
                        None => return false,
                    }
                }

                false
            };

            let mut used = Used::default();

            for process in listed.iter().filter(|process| !under_another(process.pid)) {
                used.add(Used {
                    processes: 1,
                    memory: process.memory,
                    cpu: process.cpu,
                });
            }

            used
        })
        .collect()
}

/// Memory, given in bytes, as rumbo writes it.
pub fn memory(bytes: i64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024. * 1024.))
}

/// Processor time, given in seconds.
pub fn processor_time(seconds: f64) -> String {
    if seconds >= 60. {
        let whole = seconds as i64;

        return format!("{}m{:02}s", whole / 60, whole % 60);
    }

    format!("{}s", (seconds * 10.).round() / 10.)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: i64, parent: i64, memory: i64, cpu: f64) -> ProcessUsage {
        ProcessUsage {
            pid,
            parent,
            memory,
            cpu,
            ..ProcessUsage::default()
        }
    }

    #[test]
    fn each_rumbo_counts_its_own_processes() {
        let one = [process(10, 1, 100, 1.), process(11, 10, 50, 0.5)];
        let other = [process(20, 1, 200, 2.)];

        assert_eq!(
            used(&[&one, &other]),
            vec![
                Used {
                    processes: 2,
                    memory: 150,
                    cpu: 1.5
                },
                Used {
                    processes: 1,
                    memory: 200,
                    cpu: 2.
                },
            ]
        );
    }

    /// The rumbo that started the window lists the window, the rumbos the
    /// window started, and what those started.
    #[test]
    fn a_process_another_rumbo_says_is_counted_once() {
        let first = [
            process(10, 1, 100, 1.),
            process(11, 10, 400, 3.),
            process(20, 11, 200, 2.),
            process(21, 20, 50, 0.),
            process(12, 10, 30, 0.),
        ];
        let started = [process(20, 11, 200, 2.), process(21, 20, 50, 0.)];

        let counted = used(&[&first, &started]);

        assert_eq!(
            counted[0],
            Used {
                processes: 3,
                memory: 530,
                cpu: 4.
            }
        );
        assert_eq!(
            counted[1],
            Used {
                processes: 2,
                memory: 250,
                cpu: 2.
            }
        );
    }

    #[test]
    fn a_rumbo_that_said_nothing_uses_nothing() {
        let none: [ProcessUsage; 0] = [];

        assert_eq!(used(&[&none]), vec![Used::default()]);
    }

    #[test]
    fn the_figures_read_as_rumbo_writes_them() {
        assert_eq!(memory(25 * 1024 * 1024 + 512 * 1024), "25.5 MB");
        assert_eq!(processor_time(0.), "0s");
        assert_eq!(processor_time(1.26), "1.3s");
        assert_eq!(processor_time(125.4), "2m05s");
    }
}
