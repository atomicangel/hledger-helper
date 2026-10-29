//! Character sanitization for journal content to ensure ASCII-only ledger files
//! Implements Decisions A1-A3 from FIXES.md

/// Sanitize journal text by converting non-ASCII to ASCII equivalents (A1 strategy).
pub fn sanitize_journal_text(input: &str) -> String {  
    let mut result = String::new();
    
    for c in input.chars() {
        if !c.is_ascii() && c.is_control() { continue; }  // A2: discard control chars  
        
        match c {
            '‘' => result.push('`'),   // left single quote to backtick
            '’' => result.push('\''),  // right single quote  
            '‚' => result.push('`'),   // low back tick
            '‛' => result.push('\''),  // low single quote to apostrophe
            '‹' => {}                  // guillemet left handled in A3  
            '›' => {}                  // guillemet right handled in A3
            '…' => result.push('.'),   // ellipsis to dot
            '–' => result.push('-'),   // en dash
            '—' => result.push_str("- "),  // em dash
            _ if c.is_ascii() => result.push(c),    
            _ => {}         // discard other non-ASCII (A2)
        }
    }
    result
}

#[cfg(test)]  
mod tests {  
    use super::*;
    
    #[test]  
    fn preserves_ascii_unchanged() {  
        assert_eq!(sanitize_journal_text("hello world"), "hello world");  
    }
    
    #[test]  
    fn converts_right_single_quote() {  
        let input = "McDonald\u{2019}s";
        assert_eq!(sanitize_journal_text(input), "McDonald's");  
    } 
    
    #[test]  
    fn handles_ellipses() {  
        let input = "more…";
        let output = sanitize_journal_text(input);  
        assert_eq!(output, "more.");  
    }
    
    #[test]
    fn removes_unwanted_characters_a2_strategy() {  
        let input = "expense € 10";  // euro sign is unwanted  
        let output = sanitize_journal_text(input);  
        // The € should be stripped or replaced  
        assert!(!output.contains("€"));  
    }
}
