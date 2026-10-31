//! `cantus-mcp`: Cantus's MCP server for clients that start a program (stdio). It reaches
//! the running Cantus through its HTTP door (ADR 0081 §9); see `cantus_mcp::bridge`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|a| a == "--help" || a == "-h" || a == "--version" || a == "-V")
    {
        print!("{}", cantus_mcp::bridge::usage());
        return;
    }
    let finder = cantus_mcp::bridge::Finder {
        dir: cantus_mcp::home::dir(),
    };
    let stdin = std::io::stdin();
    if let Err(e) = cantus_mcp::bridge::run(stdin.lock(), std::io::stdout(), finder) {
        eprintln!("cantus-mcp: {e}");
        std::process::exit(1);
    }
}
