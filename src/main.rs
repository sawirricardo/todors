use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const DATA_FILE: &str = "todo.json";

#[derive(Debug, Serialize, Deserialize)]
struct Task {
    id: u64,
    text: String,
    done: bool,
    #[serde(default = "now_utc")]
    created_at: DateTime<Utc>,
    #[serde(default)]
    completed_at: Option<DateTime<Utc>>,
    #[serde(default = "default_priority")]
    priority: Priority,
    #[serde(default)]
    subtasks: Vec<SubTask>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SubTask {
    id: u64,
    text: String,
    done: bool,
    #[serde(default = "now_utc")]
    created_at: DateTime<Utc>,
    #[serde(default)]
    completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Priority {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Priority::Low => "low",
            Priority::Medium => "medium",
            Priority::High => "high",
        };
        write!(f, "{label}")
    }
}

impl FromStr for Priority {
    type Err = io::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "low" => Ok(Priority::Low),
            "medium" => Ok(Priority::Medium),
            "high" => Ok(Priority::High),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid priority `{input}`: expected low|medium|high"),
            )),
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> io::Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };

    let mut tasks = load_tasks(DATA_FILE)?;

    match command.as_str() {
        "add" => {
            let (text, priority) = parse_add_args(args.collect())?;
            if text.trim().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "missing task text: use `add <task description>`",
                ));
            }

            let id = tasks.iter().map(|task| task.id).max().unwrap_or(0) + 1;
            tasks.push(Task {
                id,
                text,
                done: false,
                created_at: now_utc(),
                completed_at: None,
                priority,
                subtasks: Vec::new(),
            });
            save_tasks(DATA_FILE, &tasks)?;
            println!("Added task {id}");
        }
        "list" => {
            if tasks.is_empty() {
                println!("No tasks yet.");
                return Ok(());
            }

            for task in tasks {
                let status = if task.done { "x" } else { " " };
                println!(
                    "[{status}] {}: {} [priority: {}] (created: {}, completed: {})",
                    task.id,
                    task.text,
                    task.priority,
                    task.created_at.to_rfc3339(),
                    task.completed_at
                        .map(|ts| ts.to_rfc3339())
                        .unwrap_or_else(|| "-".to_string())
                );

                for subtask in task.subtasks {
                    let sub_status = if subtask.done { "x" } else { " " };
                    println!(
                        "    [{sub_status}] {}.{}: {} (created: {}, completed: {})",
                        task.id,
                        subtask.id,
                        subtask.text,
                        subtask.created_at.to_rfc3339(),
                        subtask
                            .completed_at
                            .map(|ts| ts.to_rfc3339())
                            .unwrap_or_else(|| "-".to_string())
                    );
                }
            }
        }
        "done" => {
            let id = parse_id(args.next())?;
            let task = tasks
                .iter_mut()
                .find(|task| task.id == id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task id not found"))?;
            task.done = true;
            if task.completed_at.is_none() {
                task.completed_at = Some(now_utc());
            }
            save_tasks(DATA_FILE, &tasks)?;
            println!("Marked task {id} as done");
        }
        "remove" => {
            let (id, yes) = parse_remove_args(args.collect())?;
            let task_index = tasks
                .iter()
                .position(|task| task.id == id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task id not found"))?;

            if !yes {
                let task_text = tasks[task_index].text.clone();
                let confirmed = confirm_action(&format!("Delete task {id}: \"{task_text}\"?"))?;
                if !confirmed {
                    println!("Canceled.");
                    return Ok(());
                }
            }

            tasks.remove(task_index);
            save_tasks(DATA_FILE, &tasks)?;
            println!("Removed task {id}");
        }
        "set-priority" => {
            let id = parse_id(args.next())?;
            let priority = parse_priority_arg(args.next())?;
            let task = tasks
                .iter_mut()
                .find(|task| task.id == id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task id not found"))?;
            task.priority = priority;
            save_tasks(DATA_FILE, &tasks)?;
            println!("Set task {id} priority to {priority}");
        }
        "reorder" => {
            let id = parse_id(args.next())?;
            let new_position = parse_position(args.next())?;
            if tasks.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "no tasks to reorder",
                ));
            }

            if new_position > tasks.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "position {new_position} out of range: expected 1..={}",
                        tasks.len()
                    ),
                ));
            }

            let current_index = tasks
                .iter()
                .position(|task| task.id == id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task id not found"))?;
            let target_index = new_position - 1;

            if current_index != target_index {
                let task = tasks.remove(current_index);
                tasks.insert(target_index, task);
                save_tasks(DATA_FILE, &tasks)?;
            }
            println!("Moved task {id} to position {new_position}");
        }
        "add-subtask" => {
            let task_id = parse_id(args.next())?;
            let text = args.collect::<Vec<String>>().join(" ");
            if text.trim().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "missing subtask text: use `add-subtask <task_id> <subtask description>`",
                ));
            }

            let task = tasks
                .iter_mut()
                .find(|task| task.id == task_id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task id not found"))?;
            let subtask_id = task
                .subtasks
                .iter()
                .map(|subtask| subtask.id)
                .max()
                .unwrap_or(0)
                + 1;

            task.subtasks.push(SubTask {
                id: subtask_id,
                text,
                done: false,
                created_at: now_utc(),
                completed_at: None,
            });
            save_tasks(DATA_FILE, &tasks)?;
            println!("Added subtask {task_id}.{subtask_id}");
        }
        "done-subtask" => {
            let task_id = parse_id(args.next())?;
            let subtask_id = parse_id(args.next())?;
            let task = tasks
                .iter_mut()
                .find(|task| task.id == task_id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task id not found"))?;
            let subtask = task
                .subtasks
                .iter_mut()
                .find(|subtask| subtask.id == subtask_id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "subtask id not found"))?;
            subtask.done = true;
            if subtask.completed_at.is_none() {
                subtask.completed_at = Some(now_utc());
            }
            save_tasks(DATA_FILE, &tasks)?;
            println!("Marked subtask {task_id}.{subtask_id} as done");
        }
        "remove-subtask" => {
            let (task_id, subtask_id, yes) = parse_remove_subtask_args(args.collect())?;
            let task_index = tasks
                .iter()
                .position(|task| task.id == task_id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task id not found"))?;
            let subtask_index = tasks[task_index]
                .subtasks
                .iter()
                .position(|subtask| subtask.id == subtask_id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "subtask id not found"))?;

            if !yes {
                let subtask_text = tasks[task_index].subtasks[subtask_index].text.clone();
                let confirmed = confirm_action(&format!(
                    "Delete subtask {task_id}.{subtask_id}: \"{subtask_text}\"?"
                ))?;
                if !confirmed {
                    println!("Canceled.");
                    return Ok(());
                }
            }

            tasks[task_index].subtasks.remove(subtask_index);
            save_tasks(DATA_FILE, &tasks)?;
            println!("Removed subtask {task_id}.{subtask_id}");
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown command (use `help`)",
            ));
        }
    }

    Ok(())
}

fn load_tasks(path: impl AsRef<Path>) -> io::Result<Vec<Task>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(path)?;
    let tasks = serde_json::from_str::<Vec<Task>>(&raw).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {}: {err}", path.display()),
        )
    })?;
    Ok(tasks)
}

fn save_tasks(path: impl AsRef<Path>, tasks: &[Task]) -> io::Result<()> {
    let json = serde_json::to_string_pretty(tasks).map_err(io::Error::other)?;
    fs::write(path, json)?;
    Ok(())
}

fn parse_id(id: Option<String>) -> io::Result<u64> {
    let id = id.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing id: expected a numeric task id",
        )
    })?;

    id.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid id `{id}`: expected a number"),
        )
    })
}

fn print_usage() {
    println!("Rust Todo List");
    println!();
    println!("Usage:");
    println!("  todors add [--priority <low|medium|high>] <task text>");
    println!("  todors list");
    println!("  todors done <id>");
    println!("  todors remove <id> [--yes]");
    println!("  todors set-priority <id> <low|medium|high>");
    println!("  todors reorder <id> <position>");
    println!("  todors add-subtask <task_id> <subtask text>");
    println!("  todors done-subtask <task_id> <subtask_id>");
    println!("  todors remove-subtask <task_id> <subtask_id> [--yes]");
    println!("  todors help");
}

fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

fn default_priority() -> Priority {
    Priority::Medium
}

fn parse_add_args(args: Vec<String>) -> io::Result<(String, Priority)> {
    let mut priority = default_priority();
    let mut text_parts: Vec<String> = Vec::new();

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--priority" | "-p" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "missing priority value after --priority/-p",
                    )
                })?;
                priority = Priority::from_str(value)?;
                i += 2;
            }
            token => {
                text_parts.push(token.to_string());
                i += 1;
            }
        }
    }

    Ok((text_parts.join(" "), priority))
}

fn parse_priority_arg(priority: Option<String>) -> io::Result<Priority> {
    let raw = priority.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing priority: expected low|medium|high",
        )
    })?;
    Priority::from_str(&raw)
}

fn parse_position(position: Option<String>) -> io::Result<usize> {
    let raw = position.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing position: expected a numeric list position",
        )
    })?;

    let parsed = raw.parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid position `{raw}`: expected a positive number"),
        )
    })?;

    if parsed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "position must be >= 1",
        ));
    }

    Ok(parsed)
}

fn parse_remove_args(args: Vec<String>) -> io::Result<(u64, bool)> {
    let mut id: Option<u64> = None;
    let mut yes = false;

    for arg in args {
        if arg == "--yes" || arg == "-y" {
            yes = true;
        } else if id.is_none() {
            id = Some(parse_id(Some(arg))?);
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unexpected argument `{arg}` for remove"),
            ));
        }
    }

    let id = id.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing id: use `remove <id> [--yes]`",
        )
    })?;
    Ok((id, yes))
}

fn parse_remove_subtask_args(args: Vec<String>) -> io::Result<(u64, u64, bool)> {
    let mut task_id: Option<u64> = None;
    let mut subtask_id: Option<u64> = None;
    let mut yes = false;

    for arg in args {
        if arg == "--yes" || arg == "-y" {
            yes = true;
        } else if task_id.is_none() {
            task_id = Some(parse_id(Some(arg))?);
        } else if subtask_id.is_none() {
            subtask_id = Some(parse_id(Some(arg))?);
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unexpected argument `{arg}` for remove-subtask"),
            ));
        }
    }

    let task_id = task_id.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing task id: use `remove-subtask <task_id> <subtask_id> [--yes]`",
        )
    })?;
    let subtask_id = subtask_id.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing subtask id: use `remove-subtask <task_id> <subtask_id> [--yes]`",
        )
    })?;

    Ok((task_id, subtask_id, yes))
}

fn confirm_action(prompt: &str) -> io::Result<bool> {
    println!("{prompt} [y/N]");
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let value = input.trim().to_ascii_lowercase();
    Ok(value == "y" || value == "yes")
}
