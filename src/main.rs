mod agent;
mod app;
mod clean;
mod cli;
mod config;
mod extract;
mod paths;
mod prompt;
mod redact;
mod session;
#[cfg(test)]
mod test_util;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(app::run(&args));
}
