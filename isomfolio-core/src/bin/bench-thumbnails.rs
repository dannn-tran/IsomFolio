//! Thumbnail-generation benchmark.
//!
//! Walks a photo folder and reports where thumbnail time goes (per decode path)
//! plus how throughput scales with worker-thread count. The folder is a CLI arg
//! so no private path lives in the repo.
//!
//!   cargo run --release -p isomfolio-core --bin bench-thumbnails -- <folder> [--limit N] [--concurrency N | --sweep]
//!
//! Build in --release: a debug decode is several times slower and misleading.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use isomfolio_core::file_index::is_supported_extension;
use isomfolio_core::indexing::thumbnail::{
    create_worker_pool, generate_thumbnail_instrumented, DecodeKind, ThumbStats,
};

struct Args {
    folder: PathBuf,
    limit: Option<usize>,
    concurrency: Option<usize>, // None => sweep
}

fn parse_args() -> Result<Args, String> {
    let mut folder: Option<PathBuf> = None;
    let mut limit: Option<usize> = None;
    let mut concurrency: Option<usize> = None;
    let mut sweep = false;

    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--limit" => {
                limit = Some(it.next().ok_or("--limit needs a value")?.parse().map_err(|_| "--limit not a number")?);
            }
            "--concurrency" => {
                concurrency = Some(it.next().ok_or("--concurrency needs a value")?.parse().map_err(|_| "--concurrency not a number")?);
            }
            "--sweep" => sweep = true,
            s if s.starts_with("--") => return Err(format!("unknown flag {s}")),
            s => folder = Some(PathBuf::from(s)),
        }
    }
    let folder = folder.ok_or("usage: bench-thumbnails <folder> [--limit N] [--concurrency N | --sweep]")?;
    if sweep {
        concurrency = None;
    }
    Ok(Args { folder, limit, concurrency })
}

fn collect_images(root: &Path, out: &mut Vec<PathBuf>, limit: Option<usize>) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("skip {}: {e}", root.display());
            return;
        }
    };
    for entry in entries.filter_map(|e| e.ok()) {
        if limit.is_some_and(|n| out.len() >= n) {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_images(&path, out, limit);
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(is_supported_extension) {
            out.push(path);
        }
    }
}

fn make_temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("isomfolio-bench-{tag}-{nanos}"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn kind_name(k: DecodeKind) -> &'static str {
    match k {
        DecodeKind::JpegScaled => "JpegScaled",
        DecodeKind::RawPreview => "RawPreview",
        DecodeKind::RawThumbnail => "RawThumbnail",
        DecodeKind::RawFull => "RawFull",
        DecodeKind::General => "General",
    }
}

fn sequential_pass(files: &[PathBuf]) {
    let cache = make_temp_dir("seq");
    let cache_str = cache.to_string_lossy().into_owned();

    let mut stats: Vec<ThumbStats> = Vec::with_capacity(files.len());
    let wall = Instant::now();
    let mut failures = 0usize;
    for (i, path) in files.iter().enumerate() {
        // A unique id per path so nothing short-circuits on an existing cache file.
        match generate_thumbnail_instrumented(&cache_str, &format!("f{i}"), &path.to_string_lossy()) {
            Ok((_, s)) => stats.push(s),
            Err(e) => {
                failures += 1;
                eprintln!("FAIL {}: {e}", path.display());
            }
        }
    }
    let wall_s = wall.elapsed().as_secs_f64();
    let _ = std::fs::remove_dir_all(&cache);

    let total_bytes: u64 = stats.iter().map(|s| s.in_bytes).sum();
    println!("\n=== Sequential pass (1 thread) ===");
    println!(
        "{} images in {:.2}s  →  {:.1} img/s, {:.1} MB/s  ({} failed)",
        stats.len(),
        wall_s,
        stats.len() as f64 / wall_s,
        (total_bytes as f64 / 1_048_576.0) / wall_s,
        failures,
    );

    println!(
        "\n{:<13} {:>6} {:>9} {:>9} {:>9} {:>9}   {:>9} {:>9}",
        "kind", "count", "dec_mean", "dec_p50", "dec_p90", "dec_max", "rsz_mean", "rsz_p90"
    );
    let mut kinds = [
        DecodeKind::JpegScaled,
        DecodeKind::RawPreview,
        DecodeKind::RawThumbnail,
        DecodeKind::RawFull,
        DecodeKind::General,
    ]
    .into_iter();
    while let Some(kind) = kinds.next() {
        let group: Vec<&ThumbStats> = stats.iter().filter(|s| s.kind == kind).collect();
        if group.is_empty() {
            continue;
        }
        let mut dec: Vec<f64> = group.iter().map(|s| s.decode_ms).collect();
        let mut rsz: Vec<f64> = group.iter().map(|s| s.resize_ms).collect();
        dec.sort_by(|a, b| a.partial_cmp(b).unwrap());
        rsz.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let dec_mean = dec.iter().sum::<f64>() / dec.len() as f64;
        let rsz_mean = rsz.iter().sum::<f64>() / rsz.len() as f64;
        println!(
            "{:<13} {:>6} {:>9.1} {:>9.1} {:>9.1} {:>9.1}   {:>9.1} {:>9.1}",
            kind_name(kind),
            group.len(),
            dec_mean,
            pct(&dec, 50.0),
            pct(&dec, 90.0),
            pct(&dec, 100.0),
            rsz_mean,
            pct(&rsz, 90.0),
        );
    }
    println!("(decode/resize times in ms per image)");
}

fn concurrency_run(files: &[PathBuf], concurrency: usize) -> f64 {
    let cache = make_temp_dir(&format!("c{concurrency}"));
    let cache_str = cache.to_string_lossy().into_owned();
    let total = files.len();

    let (tx, rx) = mpsc::channel::<()>();
    let tx_fail = tx.clone();
    let pool = create_worker_pool(
        &cache_str,
        concurrency,
        move |_, _| { let _ = tx.send(()); },
        move |_, _| { let _ = tx_fail.send(()); },
    );

    let wall = Instant::now();
    for (i, path) in files.iter().enumerate() {
        pool.enqueue(&format!("f{i}"), &path.to_string_lossy(), 0);
    }
    for _ in 0..total {
        // Generous timeout: a stuck worker shouldn't hang the bench forever.
        if rx.recv_timeout(std::time::Duration::from_secs(120)).is_err() {
            eprintln!("timeout waiting for completions at concurrency {concurrency}");
            break;
        }
    }
    let wall_s = wall.elapsed().as_secs_f64();
    drop(pool);
    let _ = std::fs::remove_dir_all(&cache);
    wall_s
}

fn sweep(files: &[PathBuf]) {
    let ncores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let mut levels = vec![1, 2, 4, ncores, ncores * 2];
    levels.sort_unstable();
    levels.dedup();

    println!("\n=== Concurrency sweep ({} cores detected) ===", ncores);
    println!("{:>8} {:>9} {:>9} {:>9}", "threads", "wall_s", "img/s", "speedup");
    let mut baseline: Option<f64> = None;
    for c in levels {
        let wall_s = concurrency_run(files, c);
        let base = *baseline.get_or_insert(wall_s);
        println!(
            "{:>8} {:>9.2} {:>9.1} {:>8.2}x",
            c,
            wall_s,
            files.len() as f64 / wall_s,
            base / wall_s,
        );
    }
    println!("(rising img/s ⇒ CPU-bound, more threads help; flat ⇒ I/O-bound)");
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };

    let mut files = Vec::new();
    collect_images(&args.folder, &mut files, args.limit);
    if let Some(n) = args.limit {
        files.truncate(n);
    }
    files.sort();

    if files.is_empty() {
        eprintln!("no supported images under {}", args.folder.display());
        std::process::exit(1);
    }
    let total_bytes: u64 = files.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
    println!(
        "Folder: {}\n{} images, {:.1} MB total ({:.2} MB mean)",
        args.folder.display(),
        files.len(),
        total_bytes as f64 / 1_048_576.0,
        total_bytes as f64 / 1_048_576.0 / files.len() as f64,
    );

    match args.concurrency {
        Some(c) => {
            let wall_s = concurrency_run(&files, c);
            println!(
                "\n=== {} threads ===\n{} images in {:.2}s  →  {:.1} img/s",
                c, files.len(), wall_s, files.len() as f64 / wall_s
            );
        }
        None => {
            sequential_pass(&files);
            sweep(&files);
        }
    }
}
