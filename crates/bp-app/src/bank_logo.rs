//! Logos des comptes : conversion de l'image choisie par l'utilisateur,
//! textures et dessin.
//!
//! L'image (PNG, JPEG, WebP, GIF, BMP, ICO ou SVG) est rognée de ses marges
//! transparentes, réduite et convertie en PNG avant d'être rangée dans le
//! fichier de comptes (`bp_core::logos`). Aucun logo n'est fourni avec
//! l'application : les logos des banques sont des marques déposées.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;

use bp_core::tr;
use egui::{Color32, Rect, Vec2, vec2};
use image::RgbaImage;

/// Plus grand côté de l'image gardée dans le fichier.
const MAX_SIDE: u32 = 256;
/// Au-delà, le fichier n'est pas lu (un logo pèse quelques dizaines de ko).
const MAX_FILE: u64 = 20 * 1024 * 1024;
/// Plus grand côté des images décodées (protège la mémoire).
const MAX_DECODED_SIDE: u32 = 8192;
/// Extensions proposées par le sélecteur de fichiers.
pub const EXTENSIONS: [&str; 10] = [
    "png", "jpg", "jpeg", "svg", "svgz", "webp", "gif", "bmp", "ico", "jfif",
];

/// Lit l'image `path` et la convertit pour le fichier de comptes.
pub fn import(path: &Path) -> Result<Vec<u8>, String> {
    let too_big = || tr!("Cette image est trop lourde (20 Mo au plus).").to_string();
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_FILE) {
        return Err(too_big());
    }
    let bytes = std::fs::read(path).map_err(|e| tr!("Image illisible : {}", e))?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(too_big());
    }
    convert(&bytes)
}

/// Convertit une image en PNG rogné, de `MAX_SIDE` pixels au plus.
pub fn convert(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let unreadable = || {
        tr!("Ce fichier n'est pas une image reconnue (PNG, JPEG, WebP, GIF, BMP ou SVG).")
            .to_string()
    };
    let image = if looks_like_svg(bytes) {
        rasterize_svg(bytes).ok_or_else(unreadable)?
    } else {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| unreadable())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DECODED_SIDE);
        limits.max_image_height = Some(MAX_DECODED_SIDE);
        reader.limits(limits);
        reader.decode().map_err(|_| unreadable())?.to_rgba8()
    };
    let image = crop_transparent(&image)
        .ok_or_else(|| tr!("Cette image est entièrement transparente.").to_string())?;
    let image = fit(&image, MAX_SIDE);
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| tr!("Image illisible : {}", e))?;
    Ok(png)
}

/// SVG (texte XML) ou SVGZ (gzip) ?
fn looks_like_svg(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return true;
    }
    let head = &bytes[..bytes.len().min(4096)];
    let head = String::from_utf8_lossy(head);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    head.starts_with('<') && head.contains("<svg")
}

/// Dessine un SVG, deux fois plus grand que l'image gardée (le rognage puis
/// la réduction donnent des bords nets).
fn rasterize_svg(bytes: &[u8]) -> Option<RgbaImage> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let longest = size.width().max(size.height());
    if !(longest.is_finite() && longest > 0.0) {
        return None;
    }
    let scale = (2 * MAX_SIDE) as f32 / longest;
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia travaille en alpha prémultiplié.
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for pixel in pixmap.pixels() {
        let c = pixel.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    RgbaImage::from_raw(width, height, data)
}

/// Image sans ses marges transparentes (`None` si rien n'est visible).
fn crop_transparent(image: &RgbaImage) -> Option<RgbaImage> {
    let visible = |x: u32, y: u32| image.get_pixel(x, y).0[3] > 8;
    let (w, h) = image.dimensions();
    let top = (0..h).find(|&y| (0..w).any(|x| visible(x, y)))?;
    let bottom = (0..h).rfind(|&y| (0..w).any(|x| visible(x, y)))?;
    let left = (0..w).find(|&x| (top..=bottom).any(|y| visible(x, y)))?;
    let right = (0..w).rfind(|&x| (top..=bottom).any(|y| visible(x, y)))?;
    Some(image::imageops::crop_imm(image, left, top, right - left + 1, bottom - top + 1).to_image())
}

/// Image réduite pour tenir dans un carré de `side` pixels (jamais agrandie).
fn fit(image: &RgbaImage, side: u32) -> RgbaImage {
    let (w, h) = image.dimensions();
    let longest = w.max(h);
    if longest <= side {
        return image.clone();
    }
    let scale = side as f32 / longest as f32;
    let width = ((w as f32 * scale).round() as u32).max(1);
    let height = ((h as f32 * scale).round() as u32).max(1);
    image::imageops::resize(image, width, height, image::imageops::FilterType::Lanczos3)
}

/// Adresse des logos de banques proposés au téléchargement. Ils ne sont
/// pas fournis avec l'application : ce sont des marques de leurs
/// propriétaires, téléchargées à la demande de l'utilisateur.
pub const CATALOG_URL: &str = "https://chelly.net/bankperfect/logos/";

/// Un logo proposé au téléchargement.
pub struct CatalogEntry {
    pub name: &'static str,
    /// Fichier SVG sous [`CATALOG_URL`].
    pub file: &'static str,
    /// Mots (sans accents, en minuscules) qui désignent cette banque dans
    /// le nom d'un compte ou d'une banque.
    keywords: &'static [&'static str],
}

const fn entry(
    name: &'static str,
    file: &'static str,
    keywords: &'static [&'static str],
) -> CatalogEntry {
    CatalogEntry {
        name,
        file,
        keywords,
    }
}

/// Logos proposés, par ordre alphabétique.
pub const CATALOG: &[CatalogEntry] = &[
    entry("Allianz", "allianz.svg", &["allianz"]),
    entry(
        "American Express",
        "american_express.svg",
        &["american express", "amex"],
    ),
    entry("AXA", "axa.svg", &["axa"]),
    entry(
        "Banque Populaire",
        "banque_populaire.svg",
        &["banque populaire", "banque pop"],
    ),
    entry("BforBank", "bforbank.svg", &["bforbank", "b for bank"]),
    entry("BNP Paribas", "bnp_paribas.svg", &["bnp", "paribas"]),
    entry(
        "BoursoBank",
        "boursobank.svg",
        &["boursobank", "boursorama", "bourso"],
    ),
    entry("BRED", "bred.svg", &["bred", "bred banque populaire"]),
    entry(
        "Caisse d'Épargne",
        "caisse_epargne.svg",
        &["caisse d epargne", "caisse epargne", "ecureuil"],
    ),
    entry("CB", "cb.svg", &["carte bancaire", "carte bleue"]),
    entry("CCF", "ccf.svg", &["ccf"]),
    entry("CIC", "cic.svg", &["cic"]),
    entry(
        "Crédit Agricole",
        "credit_agricole.svg",
        &["credit agricole", "ca"],
    ),
    entry(
        "Crédit Coopératif",
        "credit_cooperatif.svg",
        &["credit cooperatif"],
    ),
    entry(
        "Crédit Mutuel",
        "credit_mutuel.svg",
        &["credit mutuel", "cmut"],
    ),
    entry(
        "Crédit Mutuel Arkéa",
        "credit_mutuel_arkea.svg",
        &["arkea", "cmb", "credit mutuel de bretagne"],
    ),
    entry("Fortuneo", "fortuneo.svg", &["fortuneo"]),
    entry("Groupama", "groupama.svg", &["groupama"]),
    entry(
        "Hello bank!",
        "hello_bank.svg",
        &["hello bank", "hellobank"],
    ),
    entry("HSBC", "hsbc.svg", &["hsbc"]),
    entry(
        "La Banque Postale",
        "banque_postale.svg",
        &["banque postale", "la poste", "lbp", "ccp"],
    ),
    entry("LCL", "lcl.svg", &["lcl", "credit lyonnais"]),
    entry("Mastercard", "mastercard.svg", &["mastercard"]),
    entry("N26", "n26.svg", &["n26"]),
    entry("Nickel", "nickel.svg", &["nickel"]),
    entry("PayPal", "paypal.svg", &["paypal"]),
    entry("Revolut", "revolut.svg", &["revolut"]),
    entry(
        "Société Générale",
        "societe_generale.svg",
        &["societe generale", "sg", "socgen"],
    ),
    entry("Trade Republic", "trade_republic.svg", &["trade republic"]),
    entry("Visa", "visa.svg", &["visa"]),
    entry("Wise", "wise.svg", &["wise"]),
];

/// Mots de `text`, en minuscules et sans accents.
fn words(text: &str) -> Vec<String> {
    let folded: String = text
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    folded.split_whitespace().map(str::to_string).collect()
}

/// Logo du catalogue qui correspond à l'un des textes (banque puis nom du
/// compte) : celui dont le mot-clé reconnu est le plus long.
pub fn suggestion(texts: &[&str]) -> Option<&'static CatalogEntry> {
    for text in texts {
        let words = words(text);
        let found = CATALOG
            .iter()
            .flat_map(|e| e.keywords.iter().map(move |k| (e, *k)))
            .filter(|(_, keyword)| {
                let keyword: Vec<&str> = keyword.split(' ').collect();
                words
                    .windows(keyword.len())
                    .any(|w| w.iter().map(String::as_str).eq(keyword.iter().copied()))
            })
            .max_by_key(|(_, keyword)| keyword.len());
        if let Some((entry, _)) = found {
            return Some(entry);
        }
    }
    None
}

/// Télécharge un logo du catalogue et le convertit pour le fichier de
/// comptes. Bloquant : à appeler hors du fil de l'interface.
pub fn download(entry: &CatalogEntry) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .build()
        .into();
    let url = format!("{CATALOG_URL}{}", entry.file);
    let mut response = agent
        .get(&url)
        .call()
        .map_err(|e| tr!("Téléchargement du logo impossible : {}", e))?;
    let bytes = response
        .body_mut()
        .read_to_vec()
        .map_err(|e| tr!("Téléchargement du logo impossible : {}", e))?;
    convert(&bytes)
}

/// Logo prêt à dessiner : la même image à plusieurs tailles, de la plus
/// grande à la plus petite (une texture très réduite à l'affichage scintille).
#[derive(Clone)]
pub struct Logo {
    levels: Vec<egui::TextureHandle>,
    /// Largeur / hauteur.
    aspect: f32,
    /// Image pleine (photo, icône d'application) : dessinée telle quelle.
    /// Sinon le logo, détouré, est posé sur une plaque blanche.
    opaque: bool,
}

impl Logo {
    fn new(ctx: &egui::Context, uid: &str, png: &[u8]) -> Option<Self> {
        let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .ok()?
            .to_rgba8();
        let (w, h) = image.dimensions();
        if w == 0 || h == 0 || w.max(h) > MAX_DECODED_SIDE {
            return None;
        }
        let clear = image.pixels().filter(|p| p.0[3] < 250).count();
        let opaque = (clear as f32) < 0.06 * (w * h) as f32;
        let mut levels = Vec::new();
        let mut side = w.max(h).min(MAX_SIDE);
        loop {
            let level = fit(&image, side);
            let size = [level.width() as usize, level.height() as usize];
            let pixels = egui::ColorImage::from_rgba_unmultiplied(size, level.as_raw());
            levels.push(ctx.load_texture(
                format!("logo-{uid}-{side}"),
                pixels,
                egui::TextureOptions::LINEAR,
            ));
            if side <= 32 {
                break;
            }
            side /= 2;
        }
        Some(Self {
            levels,
            aspect: w as f32 / h as f32,
            opaque,
        })
    }

    /// Taille de l'image tenant dans `max`, proportions gardées.
    fn fitted(&self, max: Vec2) -> Vec2 {
        if max.x / max.y > self.aspect {
            vec2(max.y * self.aspect, max.y)
        } else {
            vec2(max.x, max.x / self.aspect)
        }
    }

    /// Place prise par le logo (plaque comprise) dans une zone de `max`.
    pub fn size_in(&self, max: Vec2, padding: f32) -> Vec2 {
        if self.opaque {
            self.fitted(max)
        } else {
            self.fitted(max - Vec2::splat(2.0 * padding)) + Vec2::splat(2.0 * padding)
        }
    }

    /// Dessine le logo dans `rect` (voir [`Self::size_in`]) : image pleine
    /// aux coins arrondis, ou logo détouré centré sur une plaque blanche.
    pub fn paint(&self, painter: &egui::Painter, rect: Rect, padding: f32, radius: f32) {
        let image_rect = if self.opaque {
            Rect::from_center_size(rect.center(), self.fitted(rect.size()))
        } else {
            painter.rect_filled(rect, radius, Color32::from_gray(252));
            let inside = rect.shrink(padding);
            Rect::from_center_size(inside.center(), self.fitted(inside.size()))
        };
        // Le plus petit niveau qui couvre encore les pixels de l'écran.
        let wanted = image_rect.size().max_elem() * painter.ctx().pixels_per_point();
        let level = self
            .levels
            .iter()
            .rev()
            .find(|t| t.size()[0].max(t.size()[1]) as f32 >= wanted)
            .unwrap_or(&self.levels[0]);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let shape = egui::epaint::RectShape::filled(
            image_rect,
            if self.opaque { radius } else { 0.0 },
            Color32::WHITE,
        )
        .with_texture(level.id(), uv);
        painter.add(shape);
    }

    /// Dessine le logo au milieu de la zone `area`, aussi grand que possible.
    pub fn paint_centered(&self, painter: &egui::Painter, area: Rect, padding: f32, radius: f32) {
        let size = self.size_in(area.size(), padding);
        self.paint(
            painter,
            Rect::from_center_size(area.center(), size),
            padding,
            radius,
        );
    }
}

/// Textures des logos du fichier ouvert, refaites quand ils changent.
#[derive(Default)]
pub struct Cache {
    revision: Option<u64>,
    fingerprint: u64,
    logos: HashMap<String, Logo>,
}

impl Cache {
    /// Met les textures à jour si le document a changé depuis le dernier
    /// appel (`revision`) et que ses logos ne sont plus les mêmes.
    pub fn sync(
        &mut self,
        ctx: &egui::Context,
        document: &bp_core::model::Document,
        revision: u64,
    ) {
        if self.revision == Some(revision) {
            return;
        }
        self.revision = Some(revision);
        let raw = bp_core::feature_store::raw(document, bp_core::logos::FEATURE_KEY);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        raw.hash(&mut hasher);
        let fingerprint = hasher.finish();
        if fingerprint == self.fingerprint && (raw.is_some() || self.logos.is_empty()) {
            return;
        }
        self.fingerprint = fingerprint;
        self.logos = bp_core::logos::Logos::load(document)
            .logos
            .iter()
            .filter_map(|l| Some((l.account.clone(), Logo::new(ctx, &l.account, &l.png)?)))
            .collect();
    }

    /// Logo du compte `uid`.
    pub fn get(&self, uid: &str) -> Option<&Logo> {
        self.logos.get(uid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(png: &[u8]) -> RgbaImage {
        image::load_from_memory(png).unwrap().to_rgba8()
    }

    fn png_of(image: &RgbaImage) -> Vec<u8> {
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }

    #[test]
    fn margins_are_cropped_and_large_images_reduced() {
        // Un rectangle rouge de 600 × 300 au milieu d'une image transparente.
        let mut image = RgbaImage::new(1000, 800);
        for y in 200..500 {
            for x in 100..700 {
                image.put_pixel(x, y, image::Rgba([200, 0, 0, 255]));
            }
        }
        let logo = decode(&convert(&png_of(&image)).unwrap());
        assert_eq!(logo.dimensions(), (256, 128));
        assert_eq!(logo.get_pixel(128, 64).0, [200, 0, 0, 255]);
    }

    #[test]
    fn small_images_keep_their_size() {
        let image = RgbaImage::from_pixel(40, 20, image::Rgba([0, 80, 160, 255]));
        assert_eq!(
            decode(&convert(&png_of(&image)).unwrap()).dimensions(),
            (40, 20)
        );
    }

    #[test]
    fn svg_is_rasterized() {
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 40 20'>\
            <rect x='10' y='5' width='20' height='10' fill='#0a6'/></svg>";
        let logo = decode(&convert(svg).unwrap());
        // Rogné sur le rectangle (2:1), au plus grand format gardé.
        assert_eq!(logo.dimensions(), (256, 128));
        assert_eq!(logo.get_pixel(128, 64).0, [0x00, 0xaa, 0x66, 255]);
    }

    #[test]
    fn the_bank_of_an_account_is_recognised() {
        let name = |texts: &[&str]| suggestion(texts).map(|e| e.name);
        assert_eq!(
            name(&["Crédit Mutuel de Bretagne"]),
            Some("Crédit Mutuel Arkéa")
        );
        assert_eq!(
            name(&["CRÉDIT AGRICOLE Centre-Est"]),
            Some("Crédit Agricole")
        );
        assert_eq!(name(&["", "Compte joint SG"]), Some("Société Générale"));
        assert_eq!(name(&["Caisse d'Épargne"]), Some("Caisse d'Épargne"));
        assert_eq!(name(&["BRED Banque Populaire"]), Some("BRED"));
        // Un mot-clé ne se cherche pas à l'intérieur d'un mot.
        assert_eq!(name(&["Cicatrice", "Visage"]), None);
        // La banque passe avant le nom du compte.
        assert_eq!(name(&["LCL", "Compte Visa"]), Some("LCL"));
        // Fichiers tous différents, noms triés.
        let mut files: Vec<_> = CATALOG.iter().map(|e| e.file).collect();
        files.sort_unstable();
        files.dedup();
        assert_eq!(files.len(), CATALOG.len());
    }

    /// Chaque logo du catalogue est dans le dossier `BP_LOGO_DIR` (celui à
    /// déposer sur le site) : `cargo test -p bankperfect catalog -- --ignored`.
    #[test]
    #[ignore]
    fn catalog_files_exist() {
        let dir = std::path::PathBuf::from(std::env::var("BP_LOGO_DIR").expect("BP_LOGO_DIR"));
        for e in CATALOG {
            assert!(import(&dir.join(e.file)).is_ok(), "{}", e.file);
        }
    }

    /// Téléchargement réel (réseau) de tout le catalogue depuis le site :
    /// `cargo test -p bankperfect downloads -- --ignored`.
    #[test]
    #[ignore]
    fn downloads_the_catalog_from_the_site() {
        for e in CATALOG {
            let png = download(e).unwrap_or_else(|err| panic!("{} : {err}", e.file));
            assert!(image::load_from_memory(&png).is_ok(), "{}", e.file);
        }
    }

    #[test]
    fn other_files_are_refused() {
        assert!(convert(b"pas une image").is_err());
        assert!(convert(b"<html><body>non</body></html>").is_err());
        assert!(convert(&png_of(&RgbaImage::new(10, 10))).is_err());
    }

    /// Convertit tous les fichiers du dossier `BP_LOGO_DIR` (essai d'une
    /// collection de logos) : `cargo test -p bankperfect folder -- --ignored`.
    #[test]
    #[ignore]
    fn converts_a_folder() {
        let dir = std::env::var("BP_LOGO_DIR").expect("BP_LOGO_DIR");
        let mut failed = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e != "md")
                && let Err(e) = import(&path)
            {
                failed.push(format!("{} : {e}", path.display()));
            }
        }
        assert!(failed.is_empty(), "{failed:#?}");
    }

    #[test]
    fn textures_follow_the_document() {
        let ctx = egui::Context::default();
        let mut document = bp_core::manage::new_document("Courant", bp_core::model::Amount::ZERO);
        let uid = document.accounts[0].uid.clone();
        let mut cache = Cache::default();
        cache.sync(&ctx, &document, 0);
        assert!(cache.get(&uid).is_none());

        let wide = RgbaImage::from_pixel(200, 50, image::Rgba([0, 0, 0, 255]));
        bp_core::logos::set(&mut document, 0, convert(&png_of(&wide)).unwrap()).unwrap();
        cache.sync(&ctx, &document, 1);
        let logo = cache.get(&uid).expect("logo chargé");
        assert!(logo.opaque);
        assert_eq!(logo.size_in(vec2(80.0, 40.0), 5.0), vec2(80.0, 20.0));
        // 200, 100, 50, 25 pixels de large.
        assert_eq!(logo.levels.len(), 4);

        bp_core::logos::remove(&mut document, 0).unwrap();
        cache.sync(&ctx, &document, 2);
        assert!(cache.get(&uid).is_none());
    }
}
