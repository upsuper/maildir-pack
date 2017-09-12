use args::Args;
use indicatif::ProgressBar;

pub fn create_progress_bar(args: &Args, len: usize) -> ProgressBar {
    if args.quiet {
        ProgressBar::hidden()
    } else {
        ProgressBar::new(len as u64)
    }
}
