//! Enhanced Python Tool with Advanced Document Generation
//! 
//! Features:
//! - Professional document templates (reports, presentations, dashboards)
//! - Auto-installation of missing Python libraries
//! - Advanced error handling with retry logic
//! - Rich formatting with modern libraries
//! - Progress streaming for long operations
//! - PPTX generation with professional themes

use serde::{Deserialize, Serialize};
use std::io::Write;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Required Python packages for document generation
const REQUIRED_PACKAGES: &[&str] = &[
    "python-docx",
    "reportlab",
    "matplotlib",
    "pandas",
    "openpyxl",
    "python-pptx",
    "Pillow",
    "numpy",
    "plotly",
    "kaleido",
    "jinja2",
    "weasyprint",
    "markdown",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct PythonExecutionResult {
    pub success: bool,
    pub output: String,
    pub formatted_output: String,
    pub errors: Vec<String>,
    pub execution_time_ms: u64,
    pub files_created: Vec<String>,
    pub suggestions: Vec<String>,
}

/// Ensure required Python packages are installed
pub async fn ensure_python_packages() -> Result<(), String> {
    // Check which packages are missing
    let check_script = r#"
import importlib
import json
packages = {
    "docx": "python-docx",
    "reportlab": "reportlab", 
    "matplotlib": "matplotlib",
    "pandas": "pandas",
    "openpyxl": "openpyxl",
    "pptx": "python-pptx",
    "PIL": "Pillow",
    "numpy": "numpy",
    "plotly": "plotly",
    "jinja2": "jinja2",
    "markdown": "markdown",
}
missing = []
for module, pip_name in packages.items():
    try:
        importlib.import_module(module)
    except ImportError:
        missing.append(pip_name)
print(json.dumps(missing))
"#;
    
    let output = Command::new("python3")
        .arg("-c")
        .arg(check_script)
        .output()
        .await
        .map_err(|e| format!("Failed to check Python packages: {}", e))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    if let Ok(missing) = serde_json::from_str::<Vec<String>>(&stdout) {
        if !missing.is_empty() {
            println!("[python_tool] Installing missing packages: {:?}", missing);
            let install_result = Command::new("python3")
                .arg("-m")
                .arg("pip")
                .arg("install")
                .arg("--quiet")
                .arg("--disable-pip-version-check")
                .args(&missing)
                .output()
                .await;
            
            match install_result {
                Ok(out) => {
                    if out.status.success() {
                        println!("[python_tool] Successfully installed: {:?}", missing);
                    } else {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        println!("[python_tool] pip install partial failure: {}", stderr);
                        // Try installing one by one
                        for pkg in &missing {
                            let _ = Command::new("python3")
                                .arg("-m")
                                .arg("pip")
                                .arg("install")
                                .arg("--quiet")
                                .arg("--disable-pip-version-check")
                                .arg(pkg)
                                .output()
                                .await;
                        }
                    }
                }
                Err(e) => {
                    println!("[python_tool] pip install failed: {}", e);
                }
            }
        }
    }
    
    Ok(())
}

/// Execute Python code with enhanced capabilities
pub async fn execute_python_enhanced(
    code: &str,
    save_to: Option<&str>,
    task_type: Option<&str>,
) -> Result<PythonExecutionResult, String> {
    let start_time = std::time::Instant::now();
    
    // Auto-install missing packages before execution
    let _ = ensure_python_packages().await;
    
    // Create temporary script
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join(format!("heywork_python_{}.py", uuid::Uuid::new_v4()));
    
    // Generate enhanced wrapper code based on task type
    let wrapped_code = generate_enhanced_wrapper(code, save_to, task_type);
    
    // Write script
    let mut file = std::fs::File::create(&script_path)
        .map_err(|e| format!("Failed to create script: {}", e))?;
    file.write_all(wrapped_code.as_bytes())
        .map_err(|e| format!("Failed to write script: {}", e))?;
    
    // Execute with timeout (120 seconds for complex tasks like presentations)
    let execution = timeout(
        Duration::from_secs(120),
        execute_python_script(&script_path)
    ).await;
    
    // Clean up
    let _ = std::fs::remove_file(&script_path);
    
    let execution_time_ms = start_time.elapsed().as_millis() as u64;
    
    match execution {
        Ok(Ok(result)) => {
            // Check if there were import errors and retry with auto-install
            if result.contains("ModuleNotFoundError") || result.contains("ImportError") {
                println!("[python_tool] Import error detected, attempting auto-install and retry");
                
                // Extract module name from error
                let module_name = extract_module_from_error(&result);
                if let Some(module) = module_name {
                    let pip_name = module_to_pip_name(&module);
                    let _ = Command::new("python3")
                        .arg("-m")
                        .arg("pip")
                        .arg("install")
                        .arg("--quiet")
                        .arg("--disable-pip-version-check")
                        .arg(&pip_name)
                        .output()
                        .await;
                    
                    // Retry execution
                    let retry_script = temp_dir.join(format!("heywork_python_retry_{}.py", uuid::Uuid::new_v4()));
                    if let Ok(mut f) = std::fs::File::create(&retry_script) {
                        let _ = f.write_all(wrapped_code.as_bytes());
                        if let Ok(Ok(retry_result)) = timeout(
                            Duration::from_secs(120),
                            execute_python_script(&retry_script)
                        ).await {
                            let _ = std::fs::remove_file(&retry_script);
                            return Ok(PythonExecutionResult {
                                success: true,
                                output: retry_result.clone(),
                                formatted_output: format_output(&retry_result, task_type),
                                errors: vec![],
                                execution_time_ms: start_time.elapsed().as_millis() as u64,
                                files_created: extract_files_created(&retry_result),
                                suggestions: generate_suggestions(&retry_result, task_type),
                            });
                        }
                        let _ = std::fs::remove_file(&retry_script);
                    }
                }
            }
            
            Ok(PythonExecutionResult {
                success: true,
                output: result.clone(),
                formatted_output: format_output(&result, task_type),
                errors: vec![],
                execution_time_ms,
                files_created: extract_files_created(&result),
                suggestions: generate_suggestions(&result, task_type),
            })
        }
        Ok(Err(e)) => {
            // Check if it's a missing module error
            if e.contains("ModuleNotFoundError") || e.contains("ImportError") {
                let module = extract_module_from_error(&e);
                if let Some(m) = &module {
                    let pip_name = module_to_pip_name(m);
                    println!("[python_tool] Auto-installing {} and retrying...", pip_name);
                    
                    let _ = Command::new("python3")
                        .arg("-m")
                        .arg("pip")
                        .arg("install")
                        .arg("--quiet")
                        .arg("--disable-pip-version-check")
                        .arg(&pip_name)
                        .output()
                        .await;
                    
                    // Retry
                    let retry_script = temp_dir.join(format!("heywork_python_retry_{}.py", uuid::Uuid::new_v4()));
                    if let Ok(mut f) = std::fs::File::create(&retry_script) {
                        let wrapped = generate_enhanced_wrapper(code, save_to, task_type);
                        let _ = f.write_all(wrapped.as_bytes());
                        if let Ok(Ok(retry_result)) = timeout(
                            Duration::from_secs(120),
                            execute_python_script(&retry_script)
                        ).await {
                            let _ = std::fs::remove_file(&retry_script);
                            return Ok(PythonExecutionResult {
                                success: true,
                                output: retry_result.clone(),
                                formatted_output: format_output(&retry_result, task_type),
                                errors: vec![],
                                execution_time_ms: start_time.elapsed().as_millis() as u64,
                                files_created: extract_files_created(&retry_result),
                                suggestions: generate_suggestions(&retry_result, task_type),
                            });
                        }
                        let _ = std::fs::remove_file(&retry_script);
                    }
                }
            }
            
            let suggestions = analyze_error(&e, code);
            Ok(PythonExecutionResult {
                success: false,
                output: String::new(),
                formatted_output: format_error_output(&e),
                errors: vec![e.clone()],
                execution_time_ms,
                files_created: vec![],
                suggestions,
            })
        }
        Err(_) => {
            Ok(PythonExecutionResult {
                success: false,
                output: String::new(),
                formatted_output: "⏱️ Execution timed out (120 seconds)\n\nThe code took too long to execute. Try:\n• Processing smaller datasets\n• Using more efficient algorithms\n• Breaking into smaller chunks".to_string(),
                errors: vec!["Timeout".to_string()],
                execution_time_ms,
                files_created: vec![],
                suggestions: vec!["Optimize code for better performance".to_string()],
            })
        }
    }
}

/// Extract module name from import error
fn extract_module_from_error(error: &str) -> Option<String> {
    // Match "No module named 'xxx'" or "ModuleNotFoundError: No module named 'xxx'"
    if let Some(pos) = error.find("No module named '") {
        let start = pos + "No module named '".len();
        if let Some(end) = error[start..].find('\'') {
            let module = &error[start..start + end];
            // Get root module (e.g., "docx" from "docx.shared")
            return Some(module.split('.').next().unwrap_or(module).to_string());
        }
    }
    if let Some(pos) = error.find("No module named \"") {
        let start = pos + "No module named \"".len();
        if let Some(end) = error[start..].find('"') {
            let module = &error[start..start + end];
            return Some(module.split('.').next().unwrap_or(module).to_string());
        }
    }
    None
}

/// Map module import names to pip package names
fn module_to_pip_name(module: &str) -> String {
    match module {
        "docx" => "python-docx".to_string(),
        "pptx" => "python-pptx".to_string(),
        "PIL" | "Pillow" => "Pillow".to_string(),
        "cv2" => "opencv-python".to_string(),
        "sklearn" => "scikit-learn".to_string(),
        "yaml" => "pyyaml".to_string(),
        "bs4" => "beautifulsoup4".to_string(),
        "dotenv" => "python-dotenv".to_string(),
        _ => module.to_string(),
    }
}

async fn execute_python_script(script_path: &std::path::Path) -> Result<String, String> {
    let output = Command::new("python3")
        .arg(script_path)
        .output()
        .await
        .map_err(|e| format!("Failed to execute Python: {}", e))?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if !output.status.success() {
        return Err(format!(
            "Python exited with code {}\n\nSTDERR:\n{}\n\nSTDOUT:\n{}",
            output.status.code().unwrap_or(-1),
            stderr,
            stdout
        ));
    }
    
    // Parse JSON result if present
    if let Ok(result) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let out = result.get("output").and_then(|o| o.as_str()).unwrap_or(&stdout);
        let err = result.get("errors").and_then(|e| e.as_str()).unwrap_or("");
        
        if !err.is_empty() {
            return Err(format!("{}", err));
        }
        return Ok(out.to_string());
    }
    
    Ok(stdout.to_string())
}

fn generate_enhanced_wrapper(code: &str, _save_to: Option<&str>, task_type: Option<&str>) -> String {
    let template_helpers = generate_template_helpers(task_type);
    let user_code_indented = code.lines().map(|l| format!("    {}", l)).collect::<Vec<_>>().join("\n");
    
    let header = r##"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Hey work - Enhanced Python Execution Environment
Auto-generated wrapper with professional document helpers
"""

import sys
import os
import json
import traceback
from io import StringIO, BytesIO
from datetime import datetime
from pathlib import Path

# Enhanced output capture
class OutputCapture:
    def __init__(self):
        self.stdout = StringIO()
        self.stderr = StringIO()
        self.files_created = []
        
    def get_output(self):
        return {
            "output": self.stdout.getvalue(),
            "errors": self.stderr.getvalue(),
            "files": self.files_created
        }

capture = OutputCapture()
sys.stdout = capture.stdout
sys.stderr = capture.stderr

"##;

    let footer = r##"

# User code execution
execution_success = True
error_message = ""

try:
"##;

    let after_user_code = r##"
except Exception as e:
    execution_success = False
    error_message = str(e)
    traceback_str = traceback.format_exc()
    
    # Print structured error for parsing
    print("\n[PYTHON_ERROR]" + json.dumps({"error": error_message, "traceback": traceback_str}) + "[/PYTHON_ERROR]")

# Restore output
sys.stdout = sys.__stdout__
sys.stderr = sys.__stderr__

# Return results
result = capture.get_output()
result["success"] = execution_success
result["error_message"] = error_message

print(json.dumps(result, default=str))
"##;

    // Concatenate without format! to avoid Rust interpreting Python f-strings
    let mut result = String::new();
    result.push_str(&header);
    result.push_str(&template_helpers);
    result.push_str(&footer);
    result.push_str(&user_code_indented);
    result.push_str(&after_user_code);
    result
}

fn generate_template_helpers(_task_type: Option<&str>) -> String {
    r####"
# ===== Professional Document Helpers =====

def create_professional_report(title: str, sections: dict, output_path: str, style: str = "modern"):
    """Create a professional report with multiple sections
    
    Args:
        title: Report title
        sections: Dict of section_name -> content (str or list of paragraphs)
        output_path: Where to save the report
        style: 'modern', 'classic', 'minimal', 'executive', 'dark'
    """
    ext = os.path.splitext(output_path)[1].lower()
    
    if ext == '.html':
        return _create_html_report(title, sections, output_path, style)
    elif ext in ['.docx', '.doc']:
        return _create_word_report(title, sections, output_path, style)
    elif ext == '.pdf':
        return _create_pdf_report(title, sections, output_path, style)
    elif ext == '.md':
        return _create_markdown_report(title, sections, output_path)
    elif ext == '.pptx':
        slides = [{"title": k, "content": v} for k, v in sections.items()]
        return create_presentation(title, slides, output_path, style)
    else:
        return _create_text_report(title, sections, output_path)

def _create_html_report(title, sections, output_path, style):
    """Create modern HTML report with advanced CSS styling"""
    styles = {
        'modern': '''
            :root { --primary: #2563eb; --bg: #f8fafc; --card: #ffffff; --text: #1e293b; --muted: #64748b; }
            * { margin: 0; padding: 0; box-sizing: border-box; }
            body { font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 960px; margin: 0 auto; padding: 40px 24px; background: var(--bg); color: var(--text); line-height: 1.7; }
            h1 { font-size: 2.25rem; font-weight: 800; letter-spacing: -0.025em; margin-bottom: 8px; background: linear-gradient(135deg, #2563eb, #7c3aed); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
            h2 { font-size: 1.375rem; font-weight: 700; color: var(--text); margin-top: 0; margin-bottom: 16px; padding-bottom: 8px; border-bottom: 2px solid var(--primary); }
            .header { margin-bottom: 40px; padding-bottom: 24px; border-bottom: 1px solid #e2e8f0; }
            .timestamp { color: var(--muted); font-size: 0.875rem; font-weight: 500; }
            .section { background: var(--card); padding: 28px; margin: 24px 0; border-radius: 12px; box-shadow: 0 1px 3px rgba(0,0,0,0.04), 0 4px 12px rgba(0,0,0,0.03); border: 1px solid #e2e8f0; transition: box-shadow 0.2s ease; }
            .section:hover { box-shadow: 0 4px 16px rgba(0,0,0,0.08); }
            p { margin-bottom: 12px; }
            code { background: #f1f5f9; padding: 2px 8px; border-radius: 6px; font-size: 0.875rem; font-family: 'JetBrains Mono', 'Fira Code', monospace; }
            pre { background: #1e293b; color: #e2e8f0; padding: 20px; border-radius: 8px; overflow-x: auto; font-size: 0.875rem; margin: 16px 0; }
            table { width: 100%; border-collapse: collapse; margin: 16px 0; }
            th { background: #f1f5f9; padding: 12px 16px; text-align: left; font-weight: 600; font-size: 0.875rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--muted); }
            td { padding: 12px 16px; border-bottom: 1px solid #e2e8f0; }
            tr:hover td { background: #f8fafc; }
            ul, ol { padding-left: 24px; margin: 12px 0; }
            li { margin-bottom: 8px; }
            .badge { display: inline-block; padding: 4px 12px; border-radius: 9999px; font-size: 0.75rem; font-weight: 600; }
            .badge-blue { background: #dbeafe; color: #1d4ed8; }
            .badge-green { background: #dcfce7; color: #15803d; }
            .badge-red { background: #fee2e2; color: #b91c1c; }
            .footer { margin-top: 48px; padding-top: 24px; border-top: 1px solid #e2e8f0; color: var(--muted); font-size: 0.875rem; text-align: center; }
        ''',
        'dark': '''
            :root { --primary: #60a5fa; --bg: #0f172a; --card: #1e293b; --text: #e2e8f0; --muted: #94a3b8; }
            * { margin: 0; padding: 0; box-sizing: border-box; }
            body { font-family: 'Inter', -apple-system, sans-serif; max-width: 960px; margin: 0 auto; padding: 40px 24px; background: var(--bg); color: var(--text); line-height: 1.7; }
            h1 { font-size: 2.25rem; font-weight: 800; background: linear-gradient(135deg, #60a5fa, #a78bfa); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
            h2 { font-size: 1.375rem; font-weight: 700; color: var(--text); margin-bottom: 16px; padding-bottom: 8px; border-bottom: 2px solid var(--primary); }
            .header { margin-bottom: 40px; padding-bottom: 24px; border-bottom: 1px solid #334155; }
            .timestamp { color: var(--muted); font-size: 0.875rem; }
            .section { background: var(--card); padding: 28px; margin: 24px 0; border-radius: 12px; border: 1px solid #334155; }
            code { background: #334155; padding: 2px 8px; border-radius: 6px; }
            pre { background: #0f172a; color: #e2e8f0; padding: 20px; border-radius: 8px; border: 1px solid #334155; }
            table { width: 100%; border-collapse: collapse; margin: 16px 0; }
            th { background: #334155; padding: 12px 16px; text-align: left; font-weight: 600; color: var(--muted); }
            td { padding: 12px 16px; border-bottom: 1px solid #334155; }
            .footer { margin-top: 48px; padding-top: 24px; border-top: 1px solid #334155; color: var(--muted); text-align: center; }
        ''',
        'executive': '''
            body { font-family: 'Georgia', 'Times New Roman', serif; max-width: 800px; margin: 60px auto; padding: 0 24px; color: #1a1a1a; line-height: 1.8; }
            h1 { font-size: 2rem; font-weight: 400; text-transform: uppercase; letter-spacing: 0.1em; border-bottom: 3px double #1a1a1a; padding-bottom: 12px; }
            h2 { font-size: 1.25rem; font-weight: 700; letter-spacing: 0.05em; margin-top: 32px; margin-bottom: 16px; }
            .section { margin: 24px 0; padding: 20px 0; border-bottom: 1px solid #e5e5e5; }
            .timestamp { font-style: italic; color: #666; }
        ''',
        'classic': '''
            body { font-family: Georgia, serif; max-width: 800px; margin: 40px auto; line-height: 1.8; color: #333; padding: 0 24px; }
            h1 { font-size: 1.75rem; border-bottom: 2px solid #333; padding-bottom: 8px; }
            h2 { font-size: 1.25rem; margin-top: 24px; }
            .section { margin: 16px 0; }
            .timestamp { color: #666; }
        ''',
        'minimal': '''
            body { font-family: system-ui, sans-serif; max-width: 700px; margin: 40px auto; color: #333; padding: 0 24px; line-height: 1.6; }
            h1 { font-size: 1.5rem; }
            h2 { font-size: 1.125rem; margin-top: 20px; }
            .section { margin: 12px 0; }
            .timestamp { color: #999; font-size: 0.875rem; }
        '''
    }
    
    css = styles.get(style, styles['modern'])
    
    html = f'''<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <style>{css}</style>
</head>
<body>
    <div class="header">
        <h1>{title}</h1>
        <div class="timestamp">Generated: {datetime.now().strftime('%B %d, %Y at %I:%M %p')}</div>
    </div>
'''
    
    for section_name, content in sections.items():
        if isinstance(content, list):
            content_html = ''.join(f'<p>{p}</p>' for p in content)
        elif isinstance(content, dict):
            # Render as table
            content_html = '<table>'
            for k, v in content.items():
                content_html += f'<tr><td><strong>{k}</strong></td><td>{v}</td></tr>'
            content_html += '</table>'
        else:
            # Convert newlines to paragraphs
            paragraphs = str(content).split('\n')
            content_html = ''.join(f'<p>{p}</p>' for p in paragraphs if p.strip())
        
        html += f'''    <div class="section">
        <h2>{section_name}</h2>
        {content_html}
    </div>
'''
    
    html += '''    <div class="footer">
        Generated by Hey work
    </div>
</body>
</html>'''
    
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(html)
    
    capture.files_created.append(output_path)
    return f"Professional HTML report created: {output_path}"

def _create_word_report(title, sections, output_path, style):
    """Create Word document with professional formatting"""
    try:
        from docx import Document
        from docx.shared import Inches, Pt, RGBColor, Cm, Emu
        from docx.enum.text import WD_ALIGN_PARAGRAPH
        from docx.enum.style import WD_STYLE_TYPE
        from docx.enum.section import WD_ORIENT
        
        doc = Document()
        
        # Configure page margins
        for section in doc.sections:
            section.top_margin = Cm(2.54)
            section.bottom_margin = Cm(2.54)
            section.left_margin = Cm(2.54)
            section.right_margin = Cm(2.54)
        
        # Style configuration based on theme
        style_config = {
            'modern': {'title_color': RGBColor(37, 99, 235), 'heading_color': RGBColor(30, 41, 59), 'body_font': 'Calibri', 'title_size': Pt(28)},
            'classic': {'title_color': RGBColor(0, 0, 0), 'heading_color': RGBColor(51, 51, 51), 'body_font': 'Times New Roman', 'title_size': Pt(24)},
            'minimal': {'title_color': RGBColor(51, 51, 51), 'heading_color': RGBColor(51, 51, 51), 'body_font': 'Helvetica', 'title_size': Pt(22)},
            'executive': {'title_color': RGBColor(26, 26, 26), 'heading_color': RGBColor(26, 26, 26), 'body_font': 'Garamond', 'title_size': Pt(26)},
            'dark': {'title_color': RGBColor(37, 99, 235), 'heading_color': RGBColor(30, 41, 59), 'body_font': 'Calibri', 'title_size': Pt(28)},
        }
        
        config = style_config.get(style, style_config['modern'])
        
        # Title
        title_para = doc.add_heading(title, 0)
        title_para.alignment = WD_ALIGN_PARAGRAPH.CENTER
        for run in title_para.runs:
            run.font.color.rgb = config['title_color']
            run.font.size = config['title_size']
        
        # Subtitle/timestamp
        subtitle = doc.add_paragraph()
        subtitle.alignment = WD_ALIGN_PARAGRAPH.CENTER
        run = subtitle.add_run(f"Generated: {datetime.now().strftime('%B %d, %Y')}")
        run.font.size = Pt(11)
        run.font.color.rgb = RGBColor(100, 116, 139)
        run.font.italic = True
        
        # Add a line break
        doc.add_paragraph()
        
        # Sections
        for section_name, content in sections.items():
            heading = doc.add_heading(section_name, level=1)
            for run in heading.runs:
                run.font.color.rgb = config['heading_color']
            
            if isinstance(content, list):
                for item in content:
                    doc.add_paragraph(str(item), style='List Bullet')
            elif isinstance(content, dict):
                # Create table for dict content
                table = doc.add_table(rows=1, cols=2)
                table.style = 'Table Grid'
                hdr_cells = table.rows[0].cells
                hdr_cells[0].text = 'Key'
                hdr_cells[1].text = 'Value'
                for k, v in content.items():
                    row_cells = table.add_row().cells
                    row_cells[0].text = str(k)
                    row_cells[1].text = str(v)
            else:
                for para in str(content).split('\n'):
                    if para.strip():
                        p = doc.add_paragraph(para.strip())
                        for run in p.runs:
                            run.font.name = config['body_font']
                            run.font.size = Pt(11)
        
        doc.save(output_path)
        capture.files_created.append(output_path)
        return f"Word document created: {output_path}"
    except ImportError:
        return "python-docx not installed. Use: pip install python-docx"

def _create_pdf_report(title, sections, output_path, style):
    """Create PDF report with professional layout"""
    try:
        from reportlab.lib.pagesizes import letter, A4
        from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
        from reportlab.lib.units import inch, cm
        from reportlab.lib.colors import HexColor
        from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, PageBreak, Table, TableStyle, HRFlowable
        from reportlab.lib.enums import TA_CENTER, TA_LEFT, TA_JUSTIFY
        from reportlab.lib import colors
        
        doc = SimpleDocTemplate(
            output_path, 
            pagesize=letter,
            rightMargin=72, leftMargin=72,
            topMargin=72, bottomMargin=72
        )
        
        styles = getSampleStyleSheet()
        
        # Custom styles
        title_style = ParagraphStyle(
            'CustomTitle',
            parent=styles['Heading1'],
            fontSize=28,
            textColor=HexColor('#1e293b'),
            spaceAfter=6,
            alignment=TA_CENTER,
            fontName='Helvetica-Bold',
            leading=34,
        )
        
        subtitle_style = ParagraphStyle(
            'CustomSubtitle',
            parent=styles['Normal'],
            fontSize=11,
            textColor=HexColor('#64748b'),
            alignment=TA_CENTER,
            spaceAfter=24,
            fontName='Helvetica-Oblique',
        )
        
        heading_style = ParagraphStyle(
            'CustomHeading',
            parent=styles['Heading2'],
            fontSize=16,
            textColor=HexColor('#2563eb'),
            spaceBefore=24,
            spaceAfter=12,
            fontName='Helvetica-Bold',
            borderColor=HexColor('#2563eb'),
            borderWidth=0,
            borderPadding=0,
        )
        
        body_style = ParagraphStyle(
            'CustomBody',
            parent=styles['Normal'],
            fontSize=11,
            textColor=HexColor('#334155'),
            spaceAfter=8,
            fontName='Helvetica',
            leading=16,
            alignment=TA_JUSTIFY,
        )
        
        story = []
        
        # Title
        story.append(Paragraph(title, title_style))
        story.append(Paragraph(
            f"Generated: {datetime.now().strftime('%B %d, %Y at %I:%M %p')}",
            subtitle_style
        ))
        story.append(HRFlowable(width="80%", thickness=1, color=HexColor('#e2e8f0'), spaceBefore=4, spaceAfter=20))
        
        # Sections
        for section_name, content in sections.items():
            story.append(Paragraph(section_name, heading_style))
            
            if isinstance(content, list):
                for item in content:
                    story.append(Paragraph(f"• {item}", body_style))
            elif isinstance(content, dict):
                table_data = [[str(k), str(v)] for k, v in content.items()]
                if table_data:
                    t = Table(table_data, colWidths=[2*inch, 4*inch])
                    t.setStyle(TableStyle([
                        ('BACKGROUND', (0, 0), (-1, -1), HexColor('#f8fafc')),
                        ('TEXTCOLOR', (0, 0), (-1, -1), HexColor('#334155')),
                        ('FONTNAME', (0, 0), (0, -1), 'Helvetica-Bold'),
                        ('FONTSIZE', (0, 0), (-1, -1), 10),
                        ('GRID', (0, 0), (-1, -1), 0.5, HexColor('#e2e8f0')),
                        ('PADDING', (0, 0), (-1, -1), 8),
                        ('VALIGN', (0, 0), (-1, -1), 'MIDDLE'),
                    ]))
                    story.append(t)
            else:
                for para in str(content).split('\n'):
                    if para.strip():
                        story.append(Paragraph(para.strip(), body_style))
            
            story.append(Spacer(1, 0.15*inch))
        
        doc.build(story)
        capture.files_created.append(output_path)
        return f"PDF report created: {output_path}"
    except ImportError:
        return "reportlab not installed. Use: pip install reportlab"

def _create_markdown_report(title, sections, output_path):
    """Create Markdown report"""
    md = f"# {title}\n\n"
    md += f"*Generated: {datetime.now().strftime('%B %d, %Y at %I:%M %p')}*\n\n"
    md += "---\n\n"
    
    for section_name, content in sections.items():
        md += f"## {section_name}\n\n"
        if isinstance(content, list):
            for item in content:
                md += f"- {item}\n"
            md += "\n"
        elif isinstance(content, dict):
            md += "| Key | Value |\n|-----|-------|\n"
            for k, v in content.items():
                md += f"| {k} | {v} |\n"
            md += "\n"
        else:
            md += f"{content}\n\n"
    
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(md)
    
    capture.files_created.append(output_path)
    return f"Markdown report created: {output_path}"

def _create_text_report(title, sections, output_path):
    """Create plain text report"""
    text = "="*60 + "\n" + title + "\n" + "="*60 + "\n\n"
    text += "Generated: " + datetime.now().strftime('%B %d, %Y at %I:%M %p') + "\n\n"
    
    for section_name, content in sections.items():
        text += "\n" + "-"*40 + "\n" + section_name + "\n" + "-"*40 + "\n\n"
        if isinstance(content, list):
            for item in content:
                text += f"  • {item}\n"
        elif isinstance(content, dict):
            for k, v in content.items():
                text += f"  {k}: {v}\n"
        else:
            text += str(content) + "\n"
    
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(text)
    
    capture.files_created.append(output_path)
    return f"Text report created: {output_path}"

# ===== Advanced Data Visualization =====

def create_advanced_chart(data, chart_type='auto', title='', save_path=None, **kwargs):
    """Create publication-quality charts with Plotly or Matplotlib
    
    Args:
        data: Data to visualize (dict, list, or DataFrame)
        chart_type: 'auto', 'bar', 'line', 'scatter', 'heatmap', 'pie', 'donut', 'area', 'histogram'
        title: Chart title
        save_path: Where to save (supports .png, .html, .svg, .pdf)
        **kwargs: Additional styling options (figsize, colors, xlabel, ylabel, theme)
    """
    theme = kwargs.get('theme', 'modern')
    
    # Try Plotly first for interactive HTML charts
    if save_path and save_path.endswith('.html'):
        return _create_plotly_chart(data, chart_type, title, save_path, **kwargs)
    
    # Fall back to matplotlib for image output
    return _create_matplotlib_chart(data, chart_type, title, save_path, **kwargs)

def _create_plotly_chart(data, chart_type, title, save_path, **kwargs):
    """Create interactive chart with Plotly"""
    try:
        import plotly.graph_objects as go
        import plotly.express as px
        
        if chart_type == 'auto':
            if isinstance(data, dict) and len(data) <= 12:
                chart_type = 'bar'
            else:
                chart_type = 'line'
        
        fig = None
        if chart_type == 'bar':
            fig = go.Figure(data=[go.Bar(x=list(data.keys()), y=list(data.values()),
                marker_color='#2563eb')])
        elif chart_type == 'line':
            fig = go.Figure(data=[go.Scatter(x=list(data.keys()), y=list(data.values()),
                mode='lines+markers', line=dict(color='#2563eb', width=3))])
        elif chart_type == 'pie':
            fig = go.Figure(data=[go.Pie(labels=list(data.keys()), values=list(data.values()),
                hole=0)])
        elif chart_type == 'donut':
            fig = go.Figure(data=[go.Pie(labels=list(data.keys()), values=list(data.values()),
                hole=0.45)])
        elif chart_type == 'area':
            fig = go.Figure(data=[go.Scatter(x=list(data.keys()), y=list(data.values()),
                fill='tozeroy', line=dict(color='#2563eb'))])
        
        if fig:
            fig.update_layout(
                title=dict(text=title, font=dict(size=20, family='Inter')),
                template='plotly_white',
                font=dict(family='Inter', size=12),
                margin=dict(l=60, r=40, t=60, b=40),
            )
            fig.write_html(save_path)
            capture.files_created.append(save_path)
            return f"Interactive chart saved: {save_path}"
    except ImportError:
        return _create_matplotlib_chart(data, chart_type, title, save_path, **kwargs)

def _create_matplotlib_chart(data, chart_type, title, save_path, **kwargs):
    """Create publication-quality chart with matplotlib"""
    try:
        import matplotlib
        matplotlib.use('Agg')
        import matplotlib.pyplot as plt
        import matplotlib.patches as mpatches
        import numpy as np
        
        # Modern color palette
        colors = kwargs.get('colors', ['#2563eb', '#7c3aed', '#059669', '#dc2626', '#d97706', '#0891b2', '#4f46e5', '#be185d'])
        
        # Set professional style
        plt.rcParams.update({
            'font.family': 'sans-serif',
            'font.sans-serif': ['Inter', 'Helvetica', 'Arial'],
            'font.size': 11,
            'axes.titlesize': 16,
            'axes.labelsize': 12,
            'xtick.labelsize': 10,
            'ytick.labelsize': 10,
            'figure.facecolor': 'white',
            'axes.facecolor': '#fafafa',
            'axes.grid': True,
            'grid.alpha': 0.3,
            'grid.linestyle': '--',
        })
        
        fig, ax = plt.subplots(figsize=kwargs.get('figsize', (12, 7)), dpi=kwargs.get('dpi', 150))
        
        # Auto-detect chart type based on data
        if chart_type == 'auto':
            if isinstance(data, dict) and len(data) <= 12:
                chart_type = 'bar'
            elif isinstance(data, list) and len(data) > 2:
                chart_type = 'line'
            else:
                chart_type = 'bar'
        
        # Create chart
        if chart_type == 'bar':
            bar_colors = [colors[i % len(colors)] for i in range(len(data))]
            bars = ax.bar(list(data.keys()), list(data.values()), color=bar_colors, 
                         edgecolor='white', linewidth=0.5, width=0.7)
            ax.set_xticklabels(list(data.keys()), rotation=kwargs.get('rotation', 30), ha='right')
            # Add value labels on bars
            for bar, val in zip(bars, data.values()):
                ax.text(bar.get_x() + bar.get_width()/2., bar.get_height() + max(data.values())*0.01,
                       f'{val:,.0f}' if isinstance(val, (int, float)) else str(val),
                       ha='center', va='bottom', fontsize=9, fontweight='bold')
        elif chart_type == 'line':
            if isinstance(data, dict):
                ax.plot(list(data.keys()), list(data.values()), linewidth=2.5, marker='o', 
                       markersize=6, color=colors[0], markerfacecolor='white', markeredgewidth=2)
            else:
                ax.plot(data, linewidth=2.5, marker='o', markersize=5, color=colors[0])
        elif chart_type == 'pie':
            pie_colors = [colors[i % len(colors)] for i in range(len(data))]
            wedges, texts, autotexts = ax.pie(list(data.values()), labels=list(data.keys()), 
                autopct='%1.1f%%', startangle=90, colors=pie_colors,
                wedgeprops=dict(width=1, edgecolor='white', linewidth=2))
            for text in autotexts:
                text.set_fontweight('bold')
                text.set_fontsize(10)
        elif chart_type == 'donut':
            pie_colors = [colors[i % len(colors)] for i in range(len(data))]
            wedges, texts, autotexts = ax.pie(list(data.values()), labels=list(data.keys()),
                autopct='%1.1f%%', startangle=90, colors=pie_colors,
                wedgeprops=dict(width=0.55, edgecolor='white', linewidth=2))
        elif chart_type == 'scatter':
            if isinstance(data, dict):
                x_vals = list(range(len(data)))
                ax.scatter(x_vals, list(data.values()), c=colors[0], s=80, alpha=0.7, edgecolors='white')
                ax.set_xticks(x_vals)
                ax.set_xticklabels(list(data.keys()), rotation=30, ha='right')
        elif chart_type == 'histogram':
            values = list(data.values()) if isinstance(data, dict) else data
            ax.hist(values, bins=kwargs.get('bins', 20), color=colors[0], edgecolor='white', alpha=0.85)
        
        if chart_type not in ['pie', 'donut']:
            ax.set_title(title, fontsize=16, fontweight='bold', pad=20)
            ax.set_xlabel(kwargs.get('xlabel', ''), fontsize=12)
            ax.set_ylabel(kwargs.get('ylabel', ''), fontsize=12)
            ax.spines['top'].set_visible(False)
            ax.spines['right'].set_visible(False)
        else:
            ax.set_title(title, fontsize=16, fontweight='bold', pad=20)
        
        plt.tight_layout()
        
        if save_path:
            plt.savefig(save_path, dpi=kwargs.get('dpi', 150), bbox_inches='tight', 
                       facecolor='white', edgecolor='none')
            plt.close()
            capture.files_created.append(save_path)
            return f"Chart saved: {save_path}"
        else:
            plt.close()
            return "Chart created (no save path specified)"
            
    except ImportError as e:
        return f"Visualization libraries not installed: {e}"

# ===== Professional Presentation Builder =====

def create_presentation(title: str, slides: list, output_path: str, theme: str = 'modern'):
    """Create professional PowerPoint presentation
    
    Args:
        title: Presentation title
        slides: List of dicts with keys:
            - 'title': Slide title
            - 'content': Text content (str, list of bullet points, or dict for key-value)
            - 'layout': Optional - 'title', 'bullets', 'two_column', 'image', 'blank'
            - 'notes': Optional speaker notes
            - 'image_path': Optional image to include
        output_path: Where to save (.pptx)
        theme: 'modern', 'dark', 'minimal', 'corporate', 'creative'
    """
    try:
        from pptx import Presentation
        from pptx.util import Inches, Pt, Emu
        from pptx.dml.color import RGBColor
        from pptx.enum.text import PP_ALIGN, MSO_ANCHOR
        from pptx.enum.shapes import MSO_SHAPE
        
        prs = Presentation()
        prs.slide_width = Inches(13.333)
        prs.slide_height = Inches(7.5)
        
        # Theme configurations
        themes = {
            'modern': {
                'bg_color': RGBColor(255, 255, 255),
                'title_color': RGBColor(30, 41, 59),
                'subtitle_color': RGBColor(100, 116, 139),
                'body_color': RGBColor(51, 65, 85),
                'accent_color': RGBColor(37, 99, 235),
                'title_font': 'Calibri',
                'body_font': 'Calibri',
                'title_size': Pt(36),
                'body_size': Pt(18),
                'accent_bar': True,
            },
            'dark': {
                'bg_color': RGBColor(15, 23, 42),
                'title_color': RGBColor(226, 232, 240),
                'subtitle_color': RGBColor(148, 163, 184),
                'body_color': RGBColor(203, 213, 225),
                'accent_color': RGBColor(96, 165, 250),
                'title_font': 'Calibri',
                'body_font': 'Calibri',
                'title_size': Pt(36),
                'body_size': Pt(18),
                'accent_bar': True,
            },
            'minimal': {
                'bg_color': RGBColor(250, 250, 250),
                'title_color': RGBColor(51, 51, 51),
                'subtitle_color': RGBColor(153, 153, 153),
                'body_color': RGBColor(68, 68, 68),
                'accent_color': RGBColor(51, 51, 51),
                'title_font': 'Helvetica',
                'body_font': 'Helvetica',
                'title_size': Pt(32),
                'body_size': Pt(16),
                'accent_bar': False,
            },
            'corporate': {
                'bg_color': RGBColor(255, 255, 255),
                'title_color': RGBColor(0, 48, 87),
                'subtitle_color': RGBColor(100, 130, 160),
                'body_color': RGBColor(51, 65, 85),
                'accent_color': RGBColor(0, 82, 136),
                'title_font': 'Arial',
                'body_font': 'Arial',
                'title_size': Pt(34),
                'body_size': Pt(17),
                'accent_bar': True,
            },
            'creative': {
                'bg_color': RGBColor(255, 255, 255),
                'title_color': RGBColor(124, 58, 237),
                'subtitle_color': RGBColor(139, 92, 246),
                'body_color': RGBColor(51, 65, 85),
                'accent_color': RGBColor(124, 58, 237),
                'title_font': 'Calibri',
                'body_font': 'Calibri',
                'title_size': Pt(38),
                'body_size': Pt(18),
                'accent_bar': True,
            },
        }
        
        t = themes.get(theme, themes['modern'])
        
        def set_slide_bg(slide, color):
            background = slide.background
            fill = background.fill
            fill.solid()
            fill.fore_color.rgb = color
        
        def add_accent_bar(slide, color, y=Inches(1.8)):
            if t['accent_bar']:
                shape = slide.shapes.add_shape(
                    MSO_SHAPE.RECTANGLE,
                    Inches(0.8), y, Inches(1.5), Inches(0.06)
                )
                shape.fill.solid()
                shape.fill.fore_color.rgb = color
                shape.line.fill.background()
        
        # ===== TITLE SLIDE =====
        title_slide = prs.slides.add_slide(prs.slide_layouts[6])  # Blank
        set_slide_bg(title_slide, t['bg_color'])
        
        # Title text
        txBox = title_slide.shapes.add_textbox(Inches(0.8), Inches(2.0), Inches(11), Inches(2))
        tf = txBox.text_frame
        tf.word_wrap = True
        p = tf.paragraphs[0]
        p.text = title
        p.font.size = Pt(44)
        p.font.bold = True
        p.font.color.rgb = t['title_color']
        p.font.name = t['title_font']
        
        # Subtitle
        txBox2 = title_slide.shapes.add_textbox(Inches(0.8), Inches(4.2), Inches(11), Inches(1))
        tf2 = txBox2.text_frame
        p2 = tf2.paragraphs[0]
        p2.text = datetime.now().strftime('%B %d, %Y')
        p2.font.size = Pt(18)
        p2.font.color.rgb = t['subtitle_color']
        p2.font.name = t['body_font']
        
        # Accent bar on title slide
        add_accent_bar(title_slide, t['accent_color'], y=Inches(3.9))
        
        # ===== CONTENT SLIDES =====
        for slide_data in slides:
            slide = prs.slides.add_slide(prs.slide_layouts[6])  # Blank
            set_slide_bg(slide, t['bg_color'])
            
            slide_title = slide_data.get('title', '')
            slide_content = slide_data.get('content', '')
            slide_layout = slide_data.get('layout', 'auto')
            slide_notes = slide_data.get('notes', '')
            
            # Slide title
            txBox = slide.shapes.add_textbox(Inches(0.8), Inches(0.5), Inches(11), Inches(1.2))
            tf = txBox.text_frame
            tf.word_wrap = True
            p = tf.paragraphs[0]
            p.text = slide_title
            p.font.size = t['title_size']
            p.font.bold = True
            p.font.color.rgb = t['title_color']
            p.font.name = t['title_font']
            
            # Accent bar
            add_accent_bar(title_slide=slide, color=t['accent_color'], y=Inches(1.6))
            
            # Content area
            content_top = Inches(2.0)
            content_width = Inches(11.5)
            content_height = Inches(4.5)
            
            if isinstance(slide_content, list):
                # Bullet points
                txBox = slide.shapes.add_textbox(Inches(0.8), content_top, content_width, content_height)
                tf = txBox.text_frame
                tf.word_wrap = True
                
                for i, bullet in enumerate(slide_content):
                    if i == 0:
                        p = tf.paragraphs[0]
                    else:
                        p = tf.add_paragraph()
                    p.text = str(bullet)
                    p.font.size = t['body_size']
                    p.font.color.rgb = t['body_color']
                    p.font.name = t['body_font']
                    p.space_after = Pt(12)
                    p.level = 0
                    
            elif isinstance(slide_content, dict):
                # Key-value pairs as formatted blocks
                txBox = slide.shapes.add_textbox(Inches(0.8), content_top, content_width, content_height)
                tf = txBox.text_frame
                tf.word_wrap = True
                
                for i, (key, value) in enumerate(slide_content.items()):
                    if i == 0:
                        p = tf.paragraphs[0]
                    else:
                        p = tf.add_paragraph()
                    
                    run_key = p.add_run()
                    run_key.text = f"{key}: "
                    run_key.font.bold = True
                    run_key.font.size = t['body_size']
                    run_key.font.color.rgb = t['accent_color']
                    run_key.font.name = t['body_font']
                    
                    run_val = p.add_run()
                    run_val.text = str(value)
                    run_val.font.size = t['body_size']
                    run_val.font.color.rgb = t['body_color']
                    run_val.font.name = t['body_font']
                    p.space_after = Pt(14)
            else:
                # Plain text content
                txBox = slide.shapes.add_textbox(Inches(0.8), content_top, content_width, content_height)
                tf = txBox.text_frame
                tf.word_wrap = True
                
                for i, para_text in enumerate(str(slide_content).split('\n')):
                    if not para_text.strip():
                        continue
                    if i == 0:
                        p = tf.paragraphs[0]
                    else:
                        p = tf.add_paragraph()
                    p.text = para_text.strip()
                    p.font.size = t['body_size']
                    p.font.color.rgb = t['body_color']
                    p.font.name = t['body_font']
                    p.space_after = Pt(10)
            
            # Speaker notes
            if slide_notes:
                notes_slide = slide.notes_slide
                notes_slide.notes_text_frame.text = slide_notes
            
            # Image
            if 'image_path' in slide_data and os.path.exists(slide_data['image_path']):
                try:
                    slide.shapes.add_picture(
                        slide_data['image_path'],
                        Inches(7), Inches(2.2),
                        height=Inches(4)
                    )
                except Exception:
                    pass
        
        # ===== THANK YOU / END SLIDE =====
        end_slide = prs.slides.add_slide(prs.slide_layouts[6])
        set_slide_bg(end_slide, t['bg_color'])
        
        txBox = end_slide.shapes.add_textbox(Inches(0.8), Inches(2.5), Inches(11), Inches(2))
        tf = txBox.text_frame
        p = tf.paragraphs[0]
        p.text = "Thank You"
        p.font.size = Pt(44)
        p.font.bold = True
        p.font.color.rgb = t['title_color']
        p.font.name = t['title_font']
        p.alignment = PP_ALIGN.CENTER
        
        p2 = tf.add_paragraph()
        p2.text = f"Generated by Hey work • {datetime.now().strftime('%B %Y')}"
        p2.font.size = Pt(14)
        p2.font.color.rgb = t['subtitle_color']
        p2.font.name = t['body_font']
        p2.alignment = PP_ALIGN.CENTER
        
        prs.save(output_path)
        capture.files_created.append(output_path)
        return f"Presentation created: {output_path} ({len(slides) + 2} slides including title and end)"
        
    except ImportError:
        return "python-pptx not installed. Use: pip install python-pptx"

# ===== Spreadsheet Builder =====

def create_spreadsheet(data: dict, output_path: str, sheet_names: list = None):
    """Create Excel spreadsheet with professional formatting
    
    Args:
        data: Dict of sheet_name -> DataFrame, list of dicts, or list of lists
        output_path: Where to save
        sheet_names: Optional list of sheet names
    """
    try:
        import pandas as pd
        from openpyxl import Workbook
        from openpyxl.styles import Font, PatternFill, Alignment, Border, Side
        from openpyxl.utils.dataframe import dataframe_to_rows
        
        if not output_path.endswith('.xlsx'):
            output_path += '.xlsx'
        
        with pd.ExcelWriter(output_path, engine='openpyxl') as writer:
            for idx, (name, df_data) in enumerate(data.items()):
                sheet_name = sheet_names[idx] if sheet_names and idx < len(sheet_names) else name[:31]
                
                if isinstance(df_data, list):
                    df = pd.DataFrame(df_data)
                elif isinstance(df_data, dict):
                    df = pd.DataFrame([df_data])
                else:
                    df = df_data
                
                df.to_excel(writer, sheet_name=sheet_name, index=False, startrow=1)
                
                worksheet = writer.sheets[sheet_name]
                
                # Header styling
                header_fill = PatternFill(start_color="2563EB", end_color="2563EB", fill_type="solid")
                header_font = Font(name='Calibri', size=11, bold=True, color="FFFFFF")
                thin_border = Border(
                    left=Side(style='thin', color='E2E8F0'),
                    right=Side(style='thin', color='E2E8F0'),
                    top=Side(style='thin', color='E2E8F0'),
                    bottom=Side(style='thin', color='E2E8F0')
                )
                
                # Write title row
                worksheet.cell(row=1, column=1, value=sheet_name)
                worksheet.cell(row=1, column=1).font = Font(name='Calibri', size=14, bold=True, color="1E293B")
                
                # Style headers (row 2)
                for col_idx, col_name in enumerate(df.columns, 1):
                    cell = worksheet.cell(row=2, column=col_idx)
                    cell.value = col_name
                    cell.fill = header_fill
                    cell.font = header_font
                    cell.alignment = Alignment(horizontal='center', vertical='center')
                    cell.border = thin_border
                
                # Style data cells
                alt_fill = PatternFill(start_color="F8FAFC", end_color="F8FAFC", fill_type="solid")
                for row_idx in range(3, worksheet.max_row + 1):
                    for col_idx in range(1, worksheet.max_column + 1):
                        cell = worksheet.cell(row=row_idx, column=col_idx)
                        cell.font = Font(name='Calibri', size=10, color="334155")
                        cell.border = thin_border
                        cell.alignment = Alignment(vertical='center')
                        if row_idx % 2 == 1:
                            cell.fill = alt_fill
                
                # Auto-adjust column widths
                for column in worksheet.columns:
                    max_length = 0
                    column_letter = column[0].column_letter
                    for cell in column:
                        try:
                            if len(str(cell.value)) > max_length:
                                max_length = len(str(cell.value))
                        except:
                            pass
                    adjusted_width = min(max_length + 4, 50)
                    worksheet.column_dimensions[column_letter].width = adjusted_width
        
        capture.files_created.append(output_path)
        return f"Excel workbook created: {output_path} ({len(data)} sheets)"
        
    except ImportError:
        return "pandas/openpyxl not installed. Use: pip install pandas openpyxl"

# ===== Data Analysis =====

def quick_analyze(data):
    """Quick statistical analysis of data"""
    try:
        import pandas as pd
        import numpy as np
        
        if isinstance(data, list):
            data = pd.DataFrame(data)
        elif isinstance(data, dict):
            data = pd.DataFrame(data)
        
        analysis = {
            'shape': str(data.shape),
            'columns': list(data.columns),
            'dtypes': {str(k): str(v) for k, v in data.dtypes.to_dict().items()},
            'summary': data.describe().to_dict(),
            'missing': data.isnull().sum().to_dict()
        }
        
        return json.dumps(analysis, indent=2, default=str)
    except ImportError:
        return "pandas required for analysis"

# ===== Dashboard Builder =====

def create_dashboard(title: str, charts: list, output_path: str, layout: str = 'grid'):
    """Create a multi-chart dashboard as HTML
    
    Args:
        title: Dashboard title
        charts: List of dicts with 'title', 'data', 'chart_type'
        output_path: .html file path
        layout: 'grid' (2-column), 'stack' (single column), 'wide' (full width)
    """
    grid_class = 'grid-2' if layout == 'grid' else 'grid-1'
    
    html = f'''<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{title}</title>
    <style>
        :root {{ --primary: #2563eb; --bg: #f1f5f9; --card: #ffffff; }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: 'Inter', -apple-system, sans-serif; background: var(--bg); padding: 24px; }}
        .header {{ text-align: center; padding: 32px 0; }}
        .header h1 {{ font-size: 2rem; color: #1e293b; }}
        .header .subtitle {{ color: #64748b; margin-top: 8px; }}
        .grid-2 {{ display: grid; grid-template-columns: repeat(2, 1fr); gap: 24px; max-width: 1200px; margin: 0 auto; }}
        .grid-1 {{ display: grid; grid-template-columns: 1fr; gap: 24px; max-width: 800px; margin: 0 auto; }}
        .card {{ background: var(--card); border-radius: 12px; padding: 24px; box-shadow: 0 1px 3px rgba(0,0,0,0.05); }}
        .card h3 {{ color: #1e293b; margin-bottom: 16px; font-size: 1.1rem; }}
        .chart-placeholder {{ background: #f8fafc; border: 2px dashed #e2e8f0; border-radius: 8px; padding: 40px; text-align: center; color: #94a3b8; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>{title}</h1>
        <p class="subtitle">Generated {datetime.now().strftime('%B %d, %Y at %I:%M %p')}</p>
    </div>
    <div class="{grid_class}">
'''
    
    for chart in charts:
        chart_title = chart.get('title', 'Chart')
        chart_data = chart.get('data', {})
        
        # Create simple SVG chart inline
        if isinstance(chart_data, dict) and chart_data:
            max_val = max(chart_data.values()) if chart_data.values() else 1
            bar_html = '<div style="display:flex;align-items:flex-end;gap:8px;height:200px;padding-top:20px;">'
            colors = ['#2563eb', '#7c3aed', '#059669', '#dc2626', '#d97706', '#0891b2']
            for i, (k, v) in enumerate(chart_data.items()):
                height_pct = (v / max_val * 100) if max_val > 0 else 0
                color = colors[i % len(colors)]
                bar_html += f'<div style="flex:1;text-align:center;"><div style="background:{color};height:{height_pct}%;min-height:4px;border-radius:6px 6px 0 0;transition:height 0.3s;"></div><div style="font-size:11px;color:#64748b;margin-top:6px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{k}</div><div style="font-size:12px;font-weight:600;color:#1e293b;">{v:,.0f}</div></div>'
            bar_html += '</div>'
            chart_html = bar_html
        else:
            chart_html = '<div class="chart-placeholder">No data</div>'
        
        html += f'''        <div class="card">
            <h3>{chart_title}</h3>
            {chart_html}
        </div>
'''
    
    html += '''    </div>
</body>
</html>'''
    
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(html)
    
    capture.files_created.append(output_path)
    return f"Dashboard created: {output_path} ({len(charts)} charts)"

"####.to_string()
}

fn format_output(output: &str, task_type: Option<&str>) -> String {
    let emoji = match task_type {
        Some("report") => "📄",
        Some("chart") | Some("viz") => "📊",
        Some("data") => "📈",
        Some("presentation") => "🎯",
        _ => "✅",
    };
    
    format!("{} {}\n\n{}", emoji, get_task_name(task_type), output)
}

fn get_task_name(task_type: Option<&str>) -> &'static str {
    match task_type {
        Some("report") => "Report Generated",
        Some("chart") | Some("viz") => "Visualization Created",
        Some("data") => "Data Analysis Complete",
        Some("presentation") => "Presentation Created",
        _ => "Python Execution Complete",
    }
}

fn format_error_output(error: &str) -> String {
    format!("❌ Python Execution Failed\n\n```\n{}\n```\n\n💡 Run in Terminal to debug:\n```\ncd /tmp && python3 script.py\n```", error)
}

fn extract_files_created(output: &str) -> Vec<String> {
    let mut files = vec![];
    
    for line in output.lines() {
        if line.contains("created:") || line.contains("saved:") {
            if let Some(path) = line.split(':').last() {
                let path = path.trim();
                if !path.is_empty() {
                    files.push(path.to_string());
                }
            }
        }
    }
    
    files
}

fn generate_suggestions(output: &str, task_type: Option<&str>) -> Vec<String> {
    let mut suggestions = vec![];
    
    if output.contains("not installed") {
        suggestions.push("💡 Auto-install attempted. If still failing, try: pip3 install python-docx reportlab matplotlib pandas openpyxl python-pptx plotly".to_string());
    }
    
    if output.contains("Permission denied") {
        suggestions.push("💡 Check file permissions or choose a different save location".to_string());
    }
    
    match task_type {
        Some("report") => {
            suggestions.push("💡 Formats: .html (modern web), .docx (Word), .pdf (print), .pptx (presentation)".to_string());
            suggestions.push("💡 Styles: 'modern', 'dark', 'executive', 'classic', 'minimal'".to_string());
        }
        Some("chart") => {
            suggestions.push("💡 Chart types: 'bar', 'line', 'pie', 'donut', 'scatter', 'area', 'histogram'".to_string());
            suggestions.push("💡 Save as .html for interactive Plotly charts".to_string());
        }
        Some("presentation") => {
            suggestions.push("💡 Themes: 'modern', 'dark', 'minimal', 'corporate', 'creative'".to_string());
            suggestions.push("💡 Include speaker notes with 'notes' key in slide data".to_string());
        }
        _ => {}
    }
    
    suggestions
}

fn analyze_error(error: &str, _code: &str) -> Vec<String> {
    let mut suggestions = vec![];
    
    if error.contains("ModuleNotFoundError") || error.contains("ImportError") {
        suggestions.push("💡 Auto-install was attempted. If still failing, manually run: pip3 install <library_name>".to_string());
    }
    
    if error.contains("Permission denied") {
        suggestions.push("💡 Choose a different save location (e.g., Desktop or Documents folder)".to_string());
    }
    
    if error.contains("SyntaxError") {
        suggestions.push("💡 Check Python syntax - ensure proper indentation and no missing colons".to_string());
    }
    
    if error.contains("FileNotFoundError") {
        suggestions.push("💡 Ensure file paths exist or use absolute paths".to_string());
    }
    
    if error.contains("Timeout") {
        suggestions.push("💡 Code took too long. Process smaller chunks or optimize algorithm".to_string());
    }
    
    if suggestions.is_empty() {
        suggestions.push("💡 Check the error message above and fix the indicated line".to_string());
    }
    
    suggestions
}

// Legacy function for backward compatibility
pub async fn execute_python_legacy(code: &str, save_to: Option<&str>) -> Result<String, String> {
    let result = execute_python_enhanced(code, save_to, None).await?;
    
    if result.success {
        Ok(result.formatted_output)
    } else {
        Err(result.errors.join("\n"))
    }
}
