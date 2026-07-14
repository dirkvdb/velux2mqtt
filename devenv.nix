{ pkgs, ... }:

{
  languages.rust = {
    enable = true;
    channel = "stable";
  };

  packages = with pkgs; [
    cargo-nextest
    just
    rust-analyzer
  ];

  scripts.build.exec = "just build";
  scripts.check.exec = "just check";
  scripts.test.exec = "just test";

  enterShell = ''
    echo "velux2mqtt migration environment ready"
    echo "Rust: $(rustc --version)"
    echo "Cargo: $(cargo --version)"
    echo ""
    echo "Run 'just check' for the Rust quality gates."
  '';

  enterTest = "just check";
}
