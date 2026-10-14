# Bundled mathematical fonts and rendering

Mori uses the MIT-licensed [RaTeX](https://github.com/erweixin/RaTeX) crates to render mathematics locally. The `ratex-katex-fonts` dependency embeds [KaTeX](https://github.com/KaTeX/KaTeX) font files in the executable. Copyright and license notices are included in:

- `assets/licenses/RaTeX-MIT.txt`
- `assets/licenses/KaTeX-MIT.txt`

Chinese fallback glyphs are read from fonts already installed on the user's operating system; system fonts are not bundled with Mori.

# Syntax highlighting

Mori bundles the MIT-licensed Tree-sitter parser and enabled language grammars for Bash, C, C++, CSS, Go, HTML, JavaScript, JSON, Python, Rust, SQL, TOML, TypeScript / TSX and YAML. Their original license and copyright notices are retained in `assets/licenses/Tree-sitter-*.txt`. The exact dependency versions are recorded in Cargo.lock.
