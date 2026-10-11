//! Mesure les temps d'ouverture et d'écriture v6 sur un fichier v5 existant.
//! Usage : cargo run --release --example bench_v6 -- <fichier.bp>

use std::time::Instant;

use bp_core::format_v6::{Protection, Unlock, write_atomic};

fn main() {
    let path = std::env::args().nth(1).expect("chemin d'un fichier .bp v5");
    let raw = std::fs::read(&path).unwrap();

    let t = Instant::now();
    let text = bp_core::codec_v5::decode(&raw).unwrap();
    println!(
        "lecture v5          : {:>8.2?}  ({} octets)",
        t.elapsed(),
        raw.len()
    );

    let t = Instant::now();
    let (protection, _recovery) = Protection::with_password("mot de passe de test").unwrap();
    println!("création mdp Argon2 : {:>8.2?}", t.elapsed());

    let out = std::env::temp_dir().join("bp-bench.bp");
    let t = Instant::now();
    let runs = 100;
    let mut size = 0;
    for _ in 0..runs {
        let sealed = protection.seal(text.as_bytes()).unwrap();
        size = sealed.len();
        write_atomic(&out, &sealed).unwrap();
    }
    println!(
        "sauvegarde v6       : {:>8.2?}  par écriture, fsync compris ({} octets)",
        t.elapsed() / runs,
        size
    );

    let t = Instant::now();
    let sealed = std::fs::read(&out).unwrap();
    Protection::open(&sealed, Unlock::Password("mot de passe de test")).unwrap();
    println!("ouverture v6 (mdp)  : {:>8.2?}", t.elapsed());
    std::fs::remove_file(out).unwrap();
}
