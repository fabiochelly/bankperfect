# Polices

`InterVariable.ttf` : Inter 4.1 (variable, axes `wght` et `opsz`), licence SIL OFL 1.1
(`Inter-OFL.txt`). La zone d'usage privé d'Unicode (U+E000–U+F8FF) en a été retirée
pour ne pas masquer les icônes Phosphor, qui occupent cette zone :

```
pyftsubset InterVariable.ttf --unicodes="U+0000-DFFF,U+F900-10FFFF" \
  --layout-features='*' --glyph-names --notdef-outline --name-IDs='*' \
  --name-languages='*' --name-legacy --hinting --output-file=InterVariable.ttf
```
