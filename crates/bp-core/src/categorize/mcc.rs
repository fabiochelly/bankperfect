//! Codes commerçant des paiements par carte (MCC, norme ISO 18245), fournis
//! par certaines banques lors de la synchronisation.

/// Plages de codes (bornes incluses) et catégorie générique.
const RANGES: &[(u16, u16, &str)] = &[
    (742, 742, "divers.animaux"),
    (1520, 1799, "logement.travaux"),
    (3000, 3350, "vacances.transport"),
    (3351, 3500, "vacances.transport"),
    (3501, 3999, "vacances.hebergement"),
    (4011, 4011, "vacances.transport"),
    (4111, 4111, "transports.commun"),
    (4112, 4112, "vacances.transport"),
    (4121, 4121, "transports.taxi"),
    (4131, 4131, "transports.commun"),
    (4411, 4411, "vacances.transport"),
    (4457, 4457, "loisirs.sports"),
    (4468, 4468, "transports.peages"),
    (4511, 4511, "vacances.transport"),
    (4582, 4582, "vacances.transport"),
    (4722, 4722, "vacances.transport"),
    (4784, 4784, "transports.peages"),
    (4812, 4812, "shopping.hightech"),
    (4814, 4816, "abonnements.telecom"),
    (4899, 4899, "abonnements.medias"),
    (4900, 4900, "logement.energie"),
    (5045, 5045, "shopping.hightech"),
    (5111, 5111, "shopping.divers"),
    (5192, 5192, "loisirs.culture"),
    (5200, 5261, "logement.travaux"),
    (5300, 5300, "alimentation.courses"),
    (5309, 5309, "shopping.divers"),
    (5310, 5311, "shopping.divers"),
    (5331, 5331, "shopping.divers"),
    (5399, 5399, "shopping.divers"),
    (5411, 5411, "alimentation.courses"),
    (5422, 5422, "alimentation.commerces"),
    (5441, 5441, "alimentation.commerces"),
    (5451, 5451, "alimentation.commerces"),
    (5462, 5462, "alimentation.commerces"),
    (5499, 5499, "alimentation.courses"),
    (5511, 5599, "transports.entretien"),
    (5541, 5542, "transports.carburant"),
    (5551, 5551, "loisirs.sports"),
    (5552, 5552, "transports.carburant"),
    (5611, 5699, "shopping.vetements"),
    (5641, 5641, "shopping.vetements"),
    (5712, 5719, "logement.equipement"),
    (5722, 5722, "shopping.hightech"),
    (5732, 5734, "shopping.hightech"),
    (5735, 5735, "loisirs.culture"),
    (5811, 5814, "alimentation.restaurants"),
    (5815, 5815, "loisirs.culture"),
    (5816, 5816, "loisirs.jeux"),
    (5817, 5818, "abonnements.logiciels"),
    (5912, 5912, "sante.pharmacie"),
    (5921, 5921, "alimentation.courses"),
    (5931, 5931, "shopping.divers"),
    (5932, 5937, "logement.equipement"),
    (5940, 5941, "loisirs.sports"),
    (5942, 5942, "loisirs.culture"),
    (5943, 5943, "enfants.ecole"),
    (5944, 5944, "divers.cadeaux"),
    (5945, 5945, "enfants.jouets"),
    (5946, 5946, "loisirs.jeux"),
    (5947, 5947, "divers.cadeaux"),
    (5948, 5949, "shopping.divers"),
    (5950, 5950, "logement.equipement"),
    (5964, 5969, "shopping.divers"),
    (5970, 5971, "loisirs.jeux"),
    (5975, 5976, "sante.dentaire"),
    (5977, 5977, "shopping.beaute"),
    (5992, 5992, "divers.cadeaux"),
    (5993, 5993, "divers.tabac"),
    (5994, 5994, "divers.tabac"),
    (5995, 5995, "divers.animaux"),
    (5996, 5996, "loisirs.sports"),
    (5999, 5999, "shopping.divers"),
    (6010, 6011, "banque.retraits"),
    (6300, 6399, "banque.assurances"),
    (6513, 6513, "logement.loyer"),
    (7011, 7011, "vacances.hebergement"),
    (7032, 7033, "vacances.hebergement"),
    (7210, 7217, "shopping.vetements"),
    (7221, 7221, "loisirs.jeux"),
    (7230, 7230, "shopping.beaute"),
    (7251, 7251, "shopping.vetements"),
    (7297, 7298, "shopping.beaute"),
    (7349, 7349, "logement.services"),
    (7512, 7519, "vacances.transport"),
    (7523, 7523, "transports.peages"),
    (7531, 7549, "transports.entretien"),
    (7832, 7833, "loisirs.sorties"),
    (7841, 7841, "loisirs.culture"),
    (7911, 7911, "enfants.activites"),
    (7922, 7929, "loisirs.sorties"),
    (7932, 7933, "loisirs.sorties"),
    (7941, 7941, "loisirs.sports"),
    (7991, 7991, "loisirs.sorties"),
    (7992, 7992, "loisirs.sports"),
    (7994, 7994, "loisirs.jeux"),
    (7995, 7995, "divers.tabac"),
    (7996, 7996, "loisirs.sorties"),
    (7997, 7997, "loisirs.sports"),
    (7998, 7999, "loisirs.sorties"),
    (8011, 8011, "sante.medecins"),
    (8021, 8021, "sante.dentaire"),
    (8031, 8031, "sante.medecins"),
    (8041, 8042, "sante.paramedical"),
    (8043, 8043, "sante.dentaire"),
    (8049, 8050, "sante.paramedical"),
    (8062, 8071, "sante.medecins"),
    (8099, 8099, "sante.paramedical"),
    (8211, 8299, "enfants.ecole"),
    (8351, 8351, "enfants.garde"),
    (8398, 8398, "divers.dons"),
    (8641, 8699, "divers.dons"),
    (9211, 9211, "impots.autres"),
    (9222, 9222, "impots.autres"),
    (9311, 9311, "impots"),
    (9399, 9399, "impots.autres"),
    (9402, 9402, "shopping.divers"),
];

/// Catégorie générique d'un code commerçant ; la plage la plus étroite
/// l'emporte.
pub fn generic_for(code: u16) -> Option<&'static str> {
    RANGES
        .iter()
        .filter(|(a, b, _)| (*a..=*b).contains(&code))
        .min_by_key(|(a, b, _)| b - a)
        .map(|(_, _, slug)| *slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_codes() {
        assert_eq!(generic_for(5411), Some("alimentation.courses"));
        assert_eq!(generic_for(5462), Some("alimentation.commerces"));
        assert_eq!(generic_for(5541), Some("transports.carburant"));
        assert_eq!(generic_for(5641), Some("shopping.vetements"));
        assert_eq!(generic_for(3500), Some("vacances.transport"));
        assert_eq!(generic_for(3600), Some("vacances.hebergement"));
        assert_eq!(generic_for(1), None);
        for (_, _, slug) in RANGES {
            assert!(super::super::taxonomy::generic(slug).is_some(), "{slug}");
        }
    }
}
