use nvim_rs::{create::async_std, Neovim, Value};
use async_trait::async_trait;
use serde_json::{self, Value as JsonValue};
use std::fs;
use std::error::Error;
use anyhow::Result;

#[derive(Debug, Clone)]
struct Cell {
    cell_type: String,
    source: Vec<String>,
    outputs: Vec<Output>,
}

#[derive(Debug, Clone)]
struct Output {
    output_type: String,
    data: Option<JsonValue>,
    text: Option<Vec<String>>,
}

#[derive(Debug)]
struct NotebookRenderer {
    nvim: Neovim<async_std::io::Stdout>,
}

impl NotebookRenderer {
    async fn new(nvim: Neovim<async_std::io::Stdout>) -> Self {
        Self { nvim }
    }

    async fn render_notebook(&self, path: &str) -> Result<()> {
        // Read and parse the notebook
        let content = fs::read_to_string(path)?;
        let notebook: JsonValue = serde_json::from_str(&content)?;
        
        // Create a new buffer
        self.nvim.command("new").await?;
        let buffer = self.nvim.get_current_buf().await?;

        if let Some(cells) = notebook["cells"].as_array() {
            for cell in cells {
                let cell_type = cell["cell_type"].as_str().unwrap_or("code");
                let source = cell["source"]
                    .as_array()
                    .unwrap_or(&Vec::new())
                    .iter()
                    .map(|s| s.as_str().unwrap_or("").to_string())
                    .collect::<Vec<String>>();

                // Render cell header
                let header = format!("─── {} cell ───", cell_type);
                buffer.set_lines(&self.nvim, -1, -1, false, vec![header]).await?;

                // Render source code
                for line in source {
                    buffer.set_lines(&self.nvim, -1, -1, false, vec![format!("│ {}", line)]).await?;
                }

                // Render outputs for code cells
                if cell_type == "code" {
                    if let Some(outputs) = cell["outputs"].as_array() {
                        for output in outputs {
                            let output_type = output["output_type"].as_str().unwrap_or("");
                            
                            match output_type {
                                "execute_result" | "display_data" => {
                                    if let Some(data) = output["data"].as_object() {
                                        // Handle different output types
                                        if let Some(text) = data.get("text/plain") {
                                            buffer
                                                .set_lines(&self.nvim, -1, -1, false, vec!["│ Output:".to_string()])
                                                .await?;
                                            let text = text.as_str().unwrap_or("").to_string();
                                            buffer
                                                .set_lines(&self.nvim, -1, -1, false, vec![format!("│ {}", text)])
                                                .await?;
                                        }
                                    }
                                }
                                "stream" => {
                                    if let Some(text) = output["text"].as_array() {
                                        buffer
                                            .set_lines(&self.nvim, -1, -1, false, vec!["│ Output:".to_string()])
                                            .await?;
                                        for line in text {
                                            let line = line.as_str().unwrap_or("").to_string();
                                            buffer
                                                .set_lines(&self.nvim, -1, -1, false, vec![format!("│ {}", line)])
                                                .await?;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Add separator between cells
                buffer.set_lines(&self.nvim, -1, -1, false, vec!["".to_string()]).await?;
            }
        }

        // Set buffer options
        self.nvim.command("set buftype=nofile").await?;
        self.nvim.command("set filetype=jupyter").await?;
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let channel = async_std::io::stdout();
    let neovim = Neovim::new(channel);
    
    let renderer = NotebookRenderer::new(neovim.clone()).await;
    
    // Register commands
    neovim
        .create_user_command(
            "JupyterOpen",
            move |nvim: Neovim<_>, args: Vec<String>| {
                let renderer = renderer.clone();
                async move {
                    if let Some(path) = args.get(0) {
                        if let Err(e) = renderer.render_notebook(path).await {
                            nvim.err_writeln(&format!("Error: {}", e)).await?;
                        }
                    }
                    Ok(())
                }
            },
            None,
        )
        .await?;

    Ok(())
}
