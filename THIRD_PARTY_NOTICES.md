# Bundled mathematical fonts and rendering

Mori uses the MIT-licensed [RaTeX](https://github.com/erweixin/RaTeX) crates to render mathematics locally. The `ratex-katex-fonts` dependency embeds [KaTeX](https://github.com/KaTeX/KaTeX) font files in the executable. Copyright and license notices are included in:

- `assets/licenses/RaTeX-MIT.txt`
- `assets/licenses/KaTeX-MIT.txt`

Chinese fallback glyphs are read from fonts already installed on the user's operating system; system fonts are not bundled with Mori.
