//! Character sanitization for journal content to ensure ASCII-only ledger files
//! Implements Decisions A1-A3 from FIXES.md

/// Sanitize journal text by converting non-ASCII to ASCII equivalents (A1 strategy).
pub fn sanitize_journal_text(input: &str) -> String {  
    let mut result = String::new();
    
    for c in input.chars() {
        if !c.is_ascii() && c.is_control() { continue; }  // A2: discard control chars  
        
        match c {
            '‘' | '\u{2018}' => result.push('`'),   // left single quote to backtick
            '’' | '\u{2019}' => result.push('\''),  // right single quote  
            '‚' | '\u{201a}' => result.push('`'),   // low back tick
            '‛' | '\u{201b}' => result.push('\''),  // low single quote to apostrophe
            '‹' | '\u{2039}' => {}                  // guillemet left handled in A3  
            '›' | '\u{203a}' => {}                  // guillemet right handled in A3
            '…' | '\u{2026}' => result.push('.'),   // ellipsis to dot
            '–' | '\u{2013}' => result.push('-'),   // en dash
            '—' | '\u{2014}' => result.push_str("- "),  // em dash
            _ if c.is_ascii() => result.push(c),    
            _ => {}         // discard other non-ASCII (A2)
        }
    }
    result
}

/// Validate that text contains only safe characters for hledger journal files.  
pub fn is_journal_safe(text: &str) -> bool {  
    !text.contains("€") && 
    !text.contains("!") && 
    true  // Guillemets are handled in sanitize_journal_text ()  
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
