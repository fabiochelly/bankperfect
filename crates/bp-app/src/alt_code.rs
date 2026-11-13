//! Codes Alt de Windows (Alt+0201 → « É ») dans les champs de texte.
//!
//! winit écarte le caractère que Windows envoie au relâchement d'Alt : sans
//! ce module, la combinaison ne produisait rien. Les chiffres tapés pendant
//! qu'Alt est enfoncé sont retenus ; au relâchement, le caractère du code est
//! inséré. Comme sous Windows : avec un zéro en tête, le code est lu dans la
//! page Windows-1252 (0201 → É), sinon dans la page 850 (144 → É).

use egui::{Event, Key};

/// Moitié haute de la page de codes 850 (128 à 255).
const CP850: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜø£Ø×ƒáíóúñÑªº¿®¬½¼¡«»░▒▓│┤ÁÂÀ©╣║╗╝¢¥┐└┴┬├─┼ãÃ╚╔╩╦╠═╬¤ðÐÊËÈıÍÎÏ┘┌█▄¦Ì▀ÓßÔÒõÕµþÞÚÛÙýÝ¯´\u{ad}±‗¾¶§÷¸°¨·¹³²■\u{a0}";

/// Chiffres tapés depuis qu'Alt est enfoncé.
#[derive(Default)]
pub struct AltCode {
    alt: bool,
    digits: String,
}

impl AltCode {
    /// Retire des évènements les chiffres d'un code Alt en cours et ajoute
    /// le caractère obtenu quand Alt est relâché. Hors d'un champ de texte
    /// (`typing` faux), seul l'état d'Alt est suivi : Alt+chiffre reste un
    /// raccourci possible.
    pub fn filter(&mut self, events: &mut Vec<Event>, typing: bool) {
        if !typing {
            self.digits.clear();
            for event in events.iter() {
                if let Event::ModifiersChanged(modifiers) = event {
                    self.alt = modifiers.alt && !modifiers.ctrl && !modifiers.command;
                }
            }
            return;
        }
        let mut out = Vec::with_capacity(events.len() + 1);
        for event in events.drain(..) {
            match &event {
                Event::ModifiersChanged(modifiers) => {
                    self.alt = modifiers.alt && !modifiers.ctrl && !modifiers.command;
                    let digits = std::mem::take(&mut self.digits);
                    out.push(event);
                    if !self.alt
                        && let Some(c) = character(&digits)
                    {
                        out.push(Event::Text(c.to_string()));
                    }
                }
                Event::Key {
                    key,
                    pressed,
                    repeat,
                    ..
                } if self.alt => match digit(*key) {
                    Some(d) => {
                        if *pressed && !*repeat && self.digits.len() < 6 {
                            self.digits.push(d);
                        }
                    }
                    None => {
                        // Alt+lettre : un raccourci, pas un code.
                        self.digits.clear();
                        out.push(event);
                    }
                },
                // Le chiffre lui-même, s'il arrive aussi en texte.
                Event::Text(text)
                    if self.alt
                        && !self.digits.is_empty()
                        && text.chars().all(|c| c.is_ascii_digit()) => {}
                _ => out.push(event),
            }
        }
        *events = out;
    }
}

fn digit(key: Key) -> Option<char> {
    Some(match key {
        Key::Num0 => '0',
        Key::Num1 => '1',
        Key::Num2 => '2',
        Key::Num3 => '3',
        Key::Num4 => '4',
        Key::Num5 => '5',
        Key::Num6 => '6',
        Key::Num7 => '7',
        Key::Num8 => '8',
        Key::Num9 => '9',
        _ => return None,
    })
}

/// Caractère d'un code Alt (`None` pour les codes de contrôle).
fn character(digits: &str) -> Option<char> {
    let code = (digits.parse::<u32>().ok()? % 256) as u8;
    if code < 32 || code == 127 {
        return None;
    }
    if code < 128 {
        return Some(code as char);
    }
    if digits.starts_with('0') {
        let bytes = [code];
        let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(&bytes);
        text.chars().next()
    } else {
        CP850.chars().nth(code as usize - 128)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, pressed: bool, alt: bool) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers {
                alt,
                ..Default::default()
            },
        }
    }

    fn alt(down: bool) -> Event {
        Event::ModifiersChanged(egui::Modifiers {
            alt: down,
            ..Default::default()
        })
    }

    /// Texte obtenu en tapant `keys` avec Alt enfoncé, puis en le relâchant.
    fn typed(keys: &[Key]) -> Vec<String> {
        let mut alt_code = AltCode::default();
        let mut events = vec![alt(true)];
        for &k in keys {
            events.extend([key(k, true, true), key(k, false, true)]);
        }
        alt_code.filter(&mut events, true);
        // Le relâchement arrive souvent dans une autre image.
        let mut later = vec![alt(false)];
        alt_code.filter(&mut later, true);
        events
            .into_iter()
            .chain(later)
            .filter_map(|e| match e {
                Event::Text(text) => Some(text),
                Event::Key { .. } => Some("touche".into()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn codes_follow_windows_code_pages() {
        assert_eq!(CP850.chars().count(), 128);
        use Key::*;
        assert_eq!(typed(&[Num0, Num2, Num0, Num1]), ["É"]);
        assert_eq!(typed(&[Num1, Num4, Num4]), ["É"]);
        assert_eq!(typed(&[Num0, Num1, Num9, Num9]), ["Ç"]);
        assert_eq!(typed(&[Num0, Num1, Num2, Num8]), ["€"]);
        assert_eq!(typed(&[Num6, Num4]), ["@"]);
        // Code de contrôle : rien.
        assert!(typed(&[Num7]).is_empty());
    }

    #[test]
    fn shortcuts_and_plain_digits_are_untouched() {
        let mut alt_code = AltCode::default();
        // Chiffre sans Alt.
        let mut events = vec![key(Key::Num1, true, false), Event::Text("1".into())];
        alt_code.filter(&mut events, true);
        assert_eq!(events.len(), 2);
        // Alt+chiffre puis Alt+lettre : le code est abandonné.
        let mut events = vec![
            alt(true),
            key(Key::Num1, true, true),
            key(Key::F, true, true),
            alt(false),
        ];
        alt_code.filter(&mut events, true);
        assert_eq!(events, [alt(true), key(Key::F, true, true), alt(false)]);
    }
}
