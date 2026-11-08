## Premiers pas

BankPerfect tient vos comptes comme un carnet de chèques : chaque ligne est une opération, et le logiciel calcule les soldes pour vous. Un fichier de comptes (extension .bp) regroupe plusieurs comptes : compte courant, compte joint, livret…

### La fenêtre

- À gauche, le panneau des comptes : le compte affiché en grand, sous forme de carte, puis les **Autres comptes** avec leur solde, et le total. Cliquez sur un compte pour l'afficher.
- En haut, les soldes du compte affiché : **En banque** (les opérations pointées), **À venir** (celles qui ne sont pas encore passées à la banque) et **Solde final**.
- Au centre, le tableau des opérations : date, mode de paiement, tiers et détail, catégorie, montant et solde après l'opération.
- En bas, la barre d'état : le nombre d'opérations, les filtres en cours, et si le fichier est **Enregistré** ou s'il reste des **Modifications non enregistrées**.

### Se déplacer dans le tableau

Les opérations les plus récentes sont en haut. Cliquez sur le titre **Date** pour inverser l'ordre ; cliquer sur un autre titre de colonne trie sur cette colonne. Un repère sépare les mois.

Faites défiler avec la molette de la souris ou avec les touches `Page préc.` et `Page suiv.`. Vous pouvez aussi faire glisser le curseur de la barre de défilement, à droite du tableau : une bulle indique le mois atteint. Cliquer dans la barre, au-dessus ou au-dessous du curseur, avance d'une page à la fois.

`Ctrl+Début` va à la première ligne de la liste, `Ctrl+Fin` à la dernière.

### Trouver une commande

Tous les menus sont en haut de la fenêtre. Si vous ne trouvez pas une commande, tapez `Ctrl+P` ou `F1` : la palette de commandes s'ouvre, tapez quelques lettres de ce que vous cherchez (par exemple « échéancier » ou « pointer ») puis `Entrée`.

### Épingler ses commandes préférées

Dans la palette, cliquez sur la punaise au bout d'une ligne : la commande s'ajoute en icône dans la barre de menus, à côté de **Filtres**. Vous composez ainsi votre propre barre d'outils. Faites glisser les icônes pour les ranger dans l'ordre voulu ; un clic droit sur une icône la retire. Quand elles ne tiennent plus à côté des menus, elles passent dans une barre juste en dessous.

> Cette aide s'ouvre à tout moment avec `Maj+F1` ou par le menu **Aide**.

## Saisir et modifier une opération

### Ajouter une opération

Cliquez sur le bouton rond **+** en bas à droite du tableau, ou tapez `Inser`. Le formulaire de saisie s'ouvre à droite (ou en bas de la fenêtre si vous avez choisi **Affichage › Saisie en bas de la fenêtre**).

1. **Quand et comment** : la date et le mode de paiement (carte, chèque, prélèvement…).
2. **Qui et pourquoi** : le tiers (le commerçant, l'organisme, la personne) et un détail facultatif (référence, numéro de chèque…).
3. **Classement** : la catégorie, et l'état de pointage si besoin.
4. Le montant : choisissez **Dépense** ou **Recette**, ou tapez simplement - ou + devant le montant.
5. Cliquez sur **Ajouter**, ou tapez `Entrée` dans le montant, ou `Ctrl+Entrée`.

Pour la date, tapez seulement le jour (15 donne le 15 du mois en cours) ou le jour et le mois (15/3). Les flèches haut et bas avancent ou reculent d'un jour, et avec `Ctrl` d'un mois. Le bouton calendrier ouvre un calendrier.

Pendant que vous tapez le tiers, BankPerfect propose ceux que vous avez déjà utilisés. `F10` recopie la dernière opération de ce tiers (mode, catégorie, montant). `F9` devine la catégorie.

Le curseur de texte saute directement à sa place. Si vous préférez qu'il glisse en douceur d'une position à l'autre, cochez **Curseur de texte animé** dans **Outils › Options…**, onglet **Affichage**.

### Modifier une opération

Double-cliquez sur la ligne, ou sélectionnez-la et tapez `Entrée`. Corrigez, puis cliquez sur **Enregistrer**. **Annuler** ou `Échap` referme le formulaire sans rien changer.

### Supprimer, copier, déplacer

- Sélectionnez une ou plusieurs lignes, puis tapez `Suppr` : BankPerfect demande confirmation.
- Pour sélectionner plusieurs lignes : `Ctrl`+clic ajoute une ligne, `Maj`+clic sélectionne toutes les lignes entre deux clics.
- `Ctrl+X`, `Ctrl+C` et `Ctrl+V` coupent, copient et collent des lignes, par exemple pour les déplacer vers un autre compte.
- Le clic droit sur une ligne ouvre un menu avec les autres actions : ventiler l'opération sur plusieurs catégories, la convertir en transfert, l'ajouter à l'échéancier…

> Une erreur ? `Ctrl+Z` annule la dernière action, même une suppression.

## Pointer et rapprocher

Pointer une opération, c'est cocher qu'elle apparaît bien sur votre relevé de banque. Une opération a trois états :

- non pointée : pas de coche ;
- pointée : une coche ;
- rapprochée : une double coche. C'est une opération pointée dont vous avez vérifié le relevé une fois pour toutes.

### Pointer

Cliquez une fois dans la colonne de pointage, à gauche de la date. Chaque clic fait passer la ligne à l'état suivant : non pointée, pointée, rapprochée, puis à nouveau non pointée. Un double-clic dans cette colonne compte pour un seul clic.

Vous pouvez aussi sélectionner une ou plusieurs lignes et taper `Espace` (ou `P`), ou choisir **Opérations › Pointer la sélection**. Quand plusieurs lignes sont sélectionnées, toutes celles qui sont dans le même état que la ligne cliquée changent ensemble.

Retirer le pointage d'une ligne rapprochée demande une confirmation : vous ne pouvez pas le faire par mégarde.

### Rapprocher

Quand toutes les lignes de votre relevé sont pointées et que le solde **En banque** correspond à celui de la banque, choisissez **Opérations › Rapprocher toutes les lignes pointées** : les lignes pointées deviennent rapprochées.

Pour les protéger complètement, cochez **Opérations › Verrouiller les lignes rapprochées** : elles ne peuvent plus être dépointées, modifiées ni supprimées tant que le verrou est mis.

### Pour aller plus loin

- **Opérations › Pointage intelligent…** : tapez le solde affiché par votre banque, cliquez sur **Chercher**, et BankPerfect propose les opérations à pointer pour retrouver ce solde. Rien n'est pointé sans votre accord.
- **Opérations › Date de pointage…** (`Ctrl+Maj+P`) change la date à laquelle une ligne a été pointée ; `F11` affiche ces dates sous la date de l'opération.
- **Affichage › Pointage à droite du montant** déplace la colonne de pointage entre le montant et le solde.
- Si vous ne vous servez pas du rapprochement, cochez **Pointage simple, sans rapprochement** dans **Outils › Options…** : une ligne est alors simplement pointée ou non.

## Rechercher et filtrer

Filtrer, c'est n'afficher qu'une partie des opérations : celles d'un mois, d'un tiers, d'une catégorie… Les autres ne sont pas supprimées, elles sont seulement cachées.

### Afficher une période

Cliquez sur le bouton **Filtres**, en haut à droite de la fenêtre, puis choisissez **Mois actuel**, **Mois précédent**, **Cette année** ou **Année dernière**.

Pour d'autres dates, choisissez **Période personnalisée…** : une ligne de filtres s'ouvre sous les titres des colonnes. Cliquez sur le bouton **Période**, sous **Date**, et tapez les dates **Du** et **Au**.

### Les filtres de colonnes

La loupe, à gauche du titre **Date**, ouvre ou ferme la ligne de filtres. Tapez dans le champ d'une colonne pour ne garder que les lignes qui contiennent ce texte :

- plusieurs mots : les lignes qui contiennent tous ces mots ;
- carrefour|auchan : l'un ou l'autre ;
- /mot : les lignes qui ne contiennent pas ce mot ;
- dans **Montant** : <0 pour les dépenses, >0 pour les recettes, 50<100 pour les montants entre 50 et 100.

Sous la loupe, un menu filtre selon le pointage : **Non pointées**, **Pointées**, **Rapprochées**…

### Quand un filtre est actif

Un bandeau coloré s'affiche en haut du tableau : il indique combien d'opérations sont affichées sur le total, et quels filtres sont en cours. Cliquez sur **Tout afficher** pour retrouver toutes vos opérations. Dans ce bandeau, chaque filtre a aussi sa petite croix pour le retirer seul.

Par défaut, les soldes du haut de la fenêtre portent alors sur les seules lignes affichées. Pour qu'ils restent calculés sur tout le compte, par exemple pour pointer face au relevé, allumez le petit cadenas en haut à droite de la tuile **Solde final** : il devient rouge, le réglage vaut pour tous les comptes, et un nouveau clic l'éteint. C'est le même que **Soldes calculés sur toutes les lignes, même quand un filtre est actif** dans **Outils › Options…**.

**Cacher les lignes inutiles** (`Ctrl+H`) n'est pas un filtre : les lignes rapprochées qu'il masque comptent toujours dans les soldes, qui restent ceux du relevé de la banque.

### Filtres enregistrés

Un filtre que vous utilisez souvent peut être gardé : **Filtres › Enregistrer le filtre actuel…**. Il apparaît ensuite dans le menu **Filtres**. **Gérer les filtres…** (`F7`) permet de les modifier.

> Les boutons 1M, 6M, 1A, 5A et Tout, au-dessus de la courbe, changent seulement la durée affichée par les courbes. Ils ne filtrent pas le tableau.

## Si des opérations semblent avoir disparu

Rassurez-vous : BankPerfect ne supprime jamais une opération sans vous demander confirmation. Le plus souvent, elle est simplement cachée. Vérifiez dans cet ordre.

### Un filtre est actif

Si un bandeau coloré est affiché en haut du tableau, ou si la barre d'état indique par exemple « 11 / 1 635 opérations », un filtre cache une partie des lignes. Cliquez sur **Tout afficher** dans le bandeau, ou choisissez **Filtres › Retirer les filtres**.

Un filtre peut se mettre en place sans que vous l'ayez cherché :

- un clic sur une semaine de la petite frise de **À venir** n'affiche que les opérations non pointées de cette semaine ;
- glisser la souris sur la courbe du solde n'affiche que la période survolée.

Dans ces deux cas, **Tout afficher** remet toutes les opérations. Un nouveau clic sur la même semaine de la frise retire aussi son filtre, et un simple clic sur la courbe retire la période.

### Les lignes inutiles sont cachées

**Affichage › Cacher les lignes inutiles** (`Ctrl+H`) cache les opérations rapprochées antérieures au mois en cours. Ce réglage est gardé d'une session à l'autre : si vous l'avez activé un jour, il l'est encore à l'ouverture suivante. Tapez `Ctrl+H` à nouveau pour tout réafficher.

### Vous n'êtes pas sur le bon compte

Le nom du compte affiché est en haut du panneau des comptes. `Ctrl+Tab` passe au compte suivant. Un compte masqué se trouve dans **Comptes désactivés**, en bas de la liste des comptes.

### Les opérations ont été archivées

**Opérations › Archiver des opérations…** remplace une période par quelques lignes de synthèse. Clic droit sur une ligne de synthèse : **Voir les opérations archivées…** ou **Restaurer les opérations archivées**.

### Elles ont vraiment été supprimées

- Tout de suite après : `Ctrl+Z` (**Édition › Annuler**) les fait revenir.
- Plus tard : **Fichier › Sauvegardes…** liste les copies faites avant chaque enregistrement. Choisissez une copie d'avant l'erreur et cliquez sur **Restaurer**. La version actuelle est elle-même sauvegardée avant d'être remplacée.

## Échéancier

L'échéancier contient vos opérations qui reviennent régulièrement : salaire, loyer, abonnements, prélèvements… BankPerfect les ajoute à vos comptes à la bonne date, et s'en sert pour calculer le solde à venir.

Ouvrez-le par **Comptes › Échéancier…** ou `Ctrl+E`. Vous pouvez aussi faire un clic droit sur une opération du tableau et choisir **Ajouter à l'échéancier…** : elle sert de modèle.

### Créer ou modifier une échéance

À gauche, la liste des échéances, triée par prochaine échéance : sa date figure en tête de chaque ligne. À l'ouverture, la liste se place sur la prochaine à partir d'aujourd'hui ; celles en retard (pas encore insérées) sont au-dessus, en orange. Cliquez sur l'une d'elles pour la modifier, ou sur **Nouvelle échéance** pour en créer une. À droite :

1. **Opération** : le tiers, le détail, le mode de paiement, la catégorie et le montant. Le mode indique s'il s'agit d'une dépense ou d'une recette.
2. **Récurrence** : la date de la **Prochaine échéance** (les petites flèches passent à l'échéance précédente ou suivante), la fréquence (**Tous les** 1 mois, 3 mois, 1 an… réglée avec le champ et les boutons - et +), et la **Dernière échéance** (laissez vide si elle n'a pas de fin). Vous pouvez reporter une échéance qui tombe un samedi, un dimanche ou un jour férié au jour ouvré suivant.
3. **Comptes** : le compte concerné, et un compte de destination s'il s'agit d'un virement entre vos comptes.

Cochez **Insérer automatiquement à l'ouverture du fichier quand l'échéance est passée** pour que l'opération s'ajoute toute seule.

Sous **Prochaines dates**, cliquez sur une date pour changer le montant de cette seule échéance (une facture plus élevée un mois donné, par exemple).

### Enregistrement

Vos modifications sont enregistrées au fur et à mesure : il n'y a pas de bouton Valider. Si vous vous êtes trompé, le bouton **Annuler les modifications**, en bas de la fenêtre, remet l'échéancier tel qu'il était à l'ouverture. Après avoir fermé la fenêtre, `Ctrl+Z` les annule aussi.

### Insérer les échéances

En bas de la fenêtre, **Insérer les échéances jusqu'au** ajoute aux comptes toutes les échéances arrivées jusqu'à la date choisie. Le bouton indique combien d'opérations seront ajoutées.

> **Solde prévisionnel**, en bas de la fenêtre, montre l'évolution de votre solde avec les échéances à venir.

## Comptes

### Ajouter un compte

Choisissez **Comptes › Gestion des comptes…** (`Ctrl+G`), restez sur l'onglet **Comptes**, cliquez sur **Nouveau**, tapez le nom du compte et son solde initial, puis **Créer le compte**. Dans la même fenêtre, vous pouvez renommer un compte, indiquer sa banque et ses coordonnées, changer l'ordre des comptes avec **Monter** et **Descendre**, ou en supprimer un.

### Masquer les anciens comptes

Un compte clos, ou celui d'une année précédente, n'a pas besoin de rester sous vos yeux. Faites un clic droit sur ce compte dans le panneau de gauche et choisissez **Masquer ce compte**.

Le compte n'est pas supprimé : il passe dans **Comptes désactivés**, une rubrique repliée en bas de la liste des comptes, et il ne compte plus dans le total. Cliquez sur **Comptes désactivés** pour le revoir.

Pour le réafficher dans la liste, ouvrez **Comptes › Gestion des comptes…**, sélectionnez-le et décochez **Compte inactif : masqué de la liste des comptes**.

### Masquer le panneau des comptes

Si vous n'utilisez qu'un compte, ou pour donner plus de place au tableau, décochez **Affichage › Panneau des comptes**. Le nom du compte s'affiche alors au-dessus des soldes : cliquez dessus pour choisir un autre compte dans la liste. Cochez à nouveau **Affichage › Panneau des comptes** pour le retrouver.

Le panneau disparaît aussi de lui-même quand la fenêtre est très étroite.

### Changer de compte au clavier

`Ctrl+Tab` affiche le compte suivant, `Ctrl+Maj+Tab` le compte précédent. Ces commandes sont aussi dans le menu **Comptes**.

### Virement entre deux comptes

**Opérations › Transfert de compte à compte…** crée en une fois la sortie sur un compte et l'entrée sur l'autre. Les deux lignes restent liées : clic droit › **Aller à la ligne liée** passe de l'une à l'autre.

## Catégories et tiers

### Les catégories

Les catégories classent vos dépenses et vos recettes (Alimentation, Logement, Salaire…), avec des sous-catégories (Logement › Loyer). Elles servent aux totaux, aux graphiques et au budget.

Pour les modifier : **Comptes › Catégories…**. Les boutons **Catégorie** et **Sous-catégorie** en ajoutent ; sélectionnez-en une pour la renommer, la déplacer (**Monter**, **Descendre**) ou la supprimer.

### Classer automatiquement

- Pendant la saisie, BankPerfect remplit la catégorie quand il la reconnaît d'après le tiers ; `F9` la devine à la demande.
- **Opérations › Catégoriser les opérations…** (`Ctrl+J`) propose une catégorie pour toutes les opérations qui n'en ont pas.
- **Opérations › Règles automatiques…** (`Ctrl+F`) applique vos propres règles, par exemple : tout tiers contenant « EDF » va dans Logement › Électricité.

### Les tiers mémorisés

Un tiers mémorisé est un modèle de saisie : en le choisissant, le mode, la catégorie et même le montant se remplissent tout seuls. Gérez-les par **Comptes › Tiers mémorisés…** (`F5`), ou ajoutez-en un depuis une opération : clic droit › **Ajouter aux tiers mémorisés…**.

### Corriger plusieurs opérations d'un coup

- **Édition › Remplacer…** (`Ctrl+R`) remplace un texte par un autre dans toutes les opérations.
- Sélectionnez plusieurs lignes puis **Opérations › Modifier le groupe…** (`Ctrl+M`) pour leur donner, par exemple, la même catégorie.

### Voir où va l'argent

Le menu **Rapports** propose les **Totaux par catégorie** (`Ctrl+T`), les totaux par tiers, des **Graphiques…** et le **Budget mensuel…** (`Ctrl+B`).

## Importer un relevé bancaire

Plutôt que de tout taper, vous pouvez récupérer les opérations depuis le site de votre banque.

### Récupérer le fichier

Sur le site de votre banque, cherchez « Télécharger », « Exporter » ou « Relevé » et choisissez le format OFX (le plus simple), QIF ou CSV. Enregistrez le fichier sur votre ordinateur.

### Importer

1. Choisissez **Fichier › Importer des opérations…** (`Ctrl+I`) et ouvrez le fichier téléchargé.
2. Vérifiez le compte de destination dans **Vers le compte**.
3. Parcourez la liste. Chaque opération est marquée **Nouvelle**, **Déjà importée** ou **Doublon probable**. Les doublons ne sont pas cochés : BankPerfect évite ainsi d'ajouter deux fois la même opération. Vous pouvez cocher ou décocher chaque ligne, ou utiliser **Tout cocher** et **Tout décocher**.
4. Choisissez l'état des opérations importées : **À pointer**, **Pointées** ou **Rapprochées**.
5. Cliquez sur **Importer**.

**Deviner les catégories** classe les nouvelles opérations d'après celles que vous avez déjà. **Créer les catégories manquantes** ajoute les catégories lues dans le fichier qui n'existent pas encore.

Si le fichier indique le solde de votre banque, la fenêtre le compare au solde **En banque après import** et affiche **Concorde** ou l'écart.

### Fichiers CSV

Un fichier CSV est un simple tableau. BankPerfect essaie de le lire seul ; sinon, indiquez dans **Format** le séparateur et l'ordre des dates, puis, dans **Colonnes**, ce que contient chaque colonne : la date, le tiers, le montant (ou bien Débit et Crédit)… L'aperçu se met à jour aussitôt.

> Les opérations importées peuvent être annulées tout de suite avec `Ctrl+Z`.

## Sauvegardes, annulation, mot de passe

### Enregistrer

Tapez `Ctrl+S` ou choisissez **Fichier › Enregistrer**. La barre d'état indique **Enregistré**, ou **Modifications non enregistrées** s'il reste quelque chose à enregistrer. En fermant BankPerfect, le logiciel vous propose d'enregistrer ; l'option **Enregistrer automatiquement à la fermeture, sans demander** (**Outils › Options…**) le fait sans poser la question.

### Annuler une erreur

`Ctrl+Z` (**Édition › Annuler**) défait la dernière action : saisie, suppression, pointage, import… Tapez-le plusieurs fois pour revenir plus loin. `Ctrl+Y` refait ce qui a été annulé. Le nombre d'actions que l'on peut annuler se règle dans **Outils › Options…** (**Niveaux d'annulation**).

### Les sauvegardes

Avant chaque enregistrement, BankPerfect garde une copie de la version précédente de votre fichier. **Fichier › Sauvegardes…** les liste avec leur date et leur heure. Choisissez-en une et cliquez sur **Restaurer** : la version actuelle est elle-même sauvegardée avant d'être remplacée, rien n'est perdu.

Le nombre de copies gardées se règle dans **Outils › Options…** (**Sauvegardes gardées par fichier**).

> Pensez aussi à copier de temps en temps votre fichier .bp sur une clé USB ou un disque externe.

### Protéger le fichier par un mot de passe

**Fichier › Protection du fichier…** : tapez un mot de passe, confirmez-le, puis cliquez sur **Protéger**. Le fichier est alors chiffré : sans le mot de passe, personne ne peut le lire.

BankPerfect affiche ensuite une clé de secours de 24 mots. Recopiez-la sur papier et rangez-la : c'est le seul moyen d'ouvrir le fichier si vous oubliez le mot de passe.

La même fenêtre permet de **Changer le mot de passe**, de créer une **Nouvelle clé de secours** ou de retirer la protection. Tant que le fichier n'est pas protégé, le panneau des comptes l'indique en bas : **Fichier non protégé**.

## Raccourcis clavier

### Les plus utiles

- `Maj+F1` : cette aide
- `Ctrl+P` ou `F1` : palette de commandes, pour trouver une commande par son nom
- `Ctrl+S` : enregistrer
- `Ctrl+Z` : annuler la dernière action ; `Ctrl+Y` : la refaire
- `Inser` : nouvelle opération
- `Entrée` : modifier l'opération sélectionnée
- `Suppr` : supprimer les opérations sélectionnées
- `Espace` ou `P` : pointer les opérations sélectionnées
- `Ctrl+E` : échéancier
- `Ctrl+H` : cacher ou réafficher les lignes inutiles

### Dans le tableau

- Flèches haut et bas : ligne précédente ou suivante
- `Page préc.` et `Page suiv.` : une page plus haut ou plus bas
- `Début` et `Fin` : haut ou bas de l'écran ; `Ctrl+Début` et `Ctrl+Fin` : première ou dernière ligne
- `Maj` avec ces touches : étendre la sélection
- `Ctrl+X`, `Ctrl+C`, `Ctrl+V` : couper, copier, coller des lignes
- `Ctrl+Tab` et `Ctrl+Maj+Tab` : compte suivant ou précédent
- `F8` : aller au formulaire de saisie

### Dans le formulaire de saisie

- `Ctrl+Entrée` : enregistrer l'opération
- `Échap` : fermer sans enregistrer
- `F9` : deviner la catégorie
- `F10` : recopier la dernière opération du tiers tapé
- Dans la date : flèches haut et bas pour un jour de plus ou de moins, avec `Ctrl` pour un mois

### Fichier

- `Ctrl+N` : nouveau fichier de comptes
- `Ctrl+O` : ouvrir un fichier
- `F12` : enregistrer sous
- `Ctrl+W` : fermer le fichier
- `Ctrl+I` : importer des opérations

### Opérations et outils

- `Ctrl+Maj+P` : date de pointage ; `F11` : afficher les dates de pointage
- `F6` : ventiler l'opération ; `F3` : champs supplémentaires
- `Ctrl+Maj+T` : transfert de compte à compte
- `Ctrl+R` : remplacer ; `Ctrl+M` : modifier le groupe
- `Ctrl+F` : règles automatiques ; `Ctrl+J` : catégoriser les opérations
- `F7` : filtres prédéfinis
- `Ctrl+G` : gestion des comptes ; `F5` : tiers mémorisés
- `Ctrl+T` : totaux par catégorie ; `Ctrl+B` : budget mensuel ; `Ctrl+Maj+G` : graphiques
- `Ctrl+L` : simulateur de crédit ; `Ctrl+D` : convertisseur de devises
- `Ctrl+Maj+C` : calculatrice ; `Ctrl+Maj+N` : notes
- `Ctrl+,` : options

## Venir de la version 8

### Ouvrir votre ancien fichier

Choisissez **Fichier › Ouvrir un fichier…** (`Ctrl+O`) et sélectionnez votre fichier .bp habituel, ou faites-le glisser dans la fenêtre. Tout est repris : comptes, opérations, pointages, catégories, tiers mémorisés, échéancier, budget.

Au premier enregistrement, le fichier passe au nouveau format, plus sûr. Votre fichier d'origine n'est pas perdu : une copie est gardée à côté, sous le nom « nom.version8.bp ». La version 8 peut toujours l'ouvrir si vous voulez revenir en arrière.

Si votre fichier avait un mot de passe, il est conservé. BankPerfect affiche alors une clé de secours de 24 mots : recopiez-la sur papier.

Si votre fichier contient des échéances d'un échéancier encore plus ancien, la fenêtre de l'échéancier le signale et propose de **Les reprendre**.

### Ce qui ne change pas

- Le pointage fonctionne de la même façon : non pointée, pointée, rapprochée, avec la touche `Espace`.
- **Cacher les lignes inutiles** (`Ctrl+H`) existe toujours et reste mémorisé d'une fois sur l'autre.
- Les opérations se saisissent avec les mêmes champs : date, mode, tiers, détail, catégorie, montant.

### Ce qui change

- Les soldes sont présentés en haut de la fenêtre : **En banque**, **À venir**, **Solde final**.
- La saisie se fait dans un formulaire à droite ; préférez-vous une barre en bas comme avant ? Cochez **Affichage › Saisie en bas de la fenêtre**.
- Pour chercher, utilisez le bouton **Filtres** en haut à droite, ou la loupe à gauche du titre **Date**.
- Les anciens scripts sont maintenant intégrés aux menus (**Rapports**, **Outils**, **Opérations**). Les anciens plugins ne fonctionnent plus ; ils sont remplacés par le menu **Extensions**.
- Une commande introuvable ? `Ctrl+P` (ou `F1`), puis tapez son nom.

> Le mode clair ou sombre, les thèmes et la taille du texte se règlent dans le menu **Affichage** et dans **Outils › Options…**.
