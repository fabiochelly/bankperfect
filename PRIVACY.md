**Français** · [English](PRIVACY.en.md)

# Confidentialité

BankPerfect est un logiciel installé sur votre ordinateur. Il n'a ni compte
utilisateur, ni serveur qui reçoive vos données, ni mesure d'audience.

## Vos données restent chez vous

Vos comptes, vos opérations et vos réglages sont enregistrés dans vos
fichiers, sur votre ordinateur. Personne d'autre n'y a accès, pas même
l'auteur du logiciel.

## Synchronisation bancaire (Enable Banking)

La synchronisation passe par [Enable Banking](https://enablebanking.com),
prestataire agréé au titre de la directive européenne DSP2 :

- vous créez votre propre application Enable Banking. Sa clé privée reste
  dans le dossier de configuration de BankPerfect, sur votre ordinateur ;
- vous vous authentifiez sur le site de votre banque. Vos identifiants
  bancaires ne passent jamais par BankPerfect ;
- BankPerfect interroge directement l'API d'Enable Banking depuis votre
  ordinateur, sans intermédiaire, et n'enregistre les opérations que dans
  votre fichier de comptes.

Le traitement de vos données par Enable Banking relève de sa propre
[politique de confidentialité](https://enablebanking.com/privacy-policy).

## Autres connexions

BankPerfect se connecte aussi à Internet dans les cas suivants :

- **Cours des devises** : téléchargés auprès de la Banque centrale
  européenne. Rien ne vous concerne dans la requête.
- **Cours des titres** : demandés à Yahoo Finance pour les codes des titres
  que vous suivez.
- **Licence** : si vous avez acheté BankPerfect, votre clé de licence est
  envoyée une fois au serveur de chelly.net, qui renvoie un certificat
  signé. Ensuite, la vérification se fait hors connexion.
- **Catégorisation par l'IA** (désactivée par défaut) : la catégorisation
  automatique fonctionne hors connexion, d'après votre historique et un
  dictionnaire intégré. Si vous activez l'option et indiquez votre propre
  clé d'API Anthropic, le bouton « Demander à l'IA » de la fenêtre
  « Catégoriser les opérations » envoie à Anthropic, et seulement à ce
  moment-là, les libellés que BankPerfect ne sait pas classer : nettoyés
  (sans chiffres, références ni dates), avec le sens et un ordre de
  grandeur du montant, et la liste de vos noms de catégories. Ni vos comptes,
  ni vos soldes, ni les montants exacts ne sont transmis. Le traitement par
  Anthropic relève de sa propre
  [politique de confidentialité](https://www.anthropic.com/legal/privacy).

## Contact

Fabio Chelly — fabio@chelly.net

---

# Privacy

Summary; full English version: [PRIVACY.en.md](PRIVACY.en.md).

BankPerfect is desktop software. It has no user accounts, no server that
receives your data and no analytics. Your accounts and transactions are
stored in your own files, on your own computer.

Bank synchronisation uses Enable Banking, a PSD2-licensed provider. Each
user registers their own Enable Banking application, and its private key
stays on their computer. Bank credentials are entered on the bank's own
website only. BankPerfect calls the Enable Banking API directly from the
user's computer and stores transactions only in the user's file.

Other connections: exchange rates (European Central Bank), security quotes
(Yahoo Finance, for the tickers the user follows), and a one-time licence
activation on chelly.net for paying users. Automatic categorisation works
offline; only if the user enables the AI option and supplies their own
Anthropic API key, clicking "Demander à l'IA" sends the labels BankPerfect
cannot classify (stripped of numbers and references), the direction and
rough size of the amount, and the user's category names to Anthropic.

Contact: Fabio Chelly — fabio@chelly.net
