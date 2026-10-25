//! Buffer de texto de alto desempenho baseado em Rope (`ropey`).
//!
//! Permite manipulação e edição de documentos gigantescos com custo $O(\log N)$
//! por operação de inserção ou remoção, sem causar realocações lineares na memória
//! e sem travar processadores legados de 2015 ou anteriores.

use crate::error::{CoreError, Result};
use ropey::Rope;
use std::fmt;

/// Buffer de texto de uma nota em edição ou visualização.
#[derive(Clone, Default)]
pub struct NoteBuffer {
    rope: Rope,
}

impl NoteBuffer {
    /// Cria um buffer vazio.
    pub fn empty() -> Self {
        Self { rope: Rope::new() }
    }

    /// Cria um buffer a partir de uma fatia de texto UTF-8.
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    /// Retorna o número total de caracteres (codepoints Unicode) no documento.
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Retorna o número total de linhas no documento.
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Retorna se o buffer está vazio.
    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Insere uma string de texto em um índice de caractere específico.
    ///
    /// # Erros
    /// Retorna `CoreError::IndexOutOfBounds` se o índice for maior que o tamanho em caracteres.
    pub fn insert(&mut self, char_idx: usize, text: &str) -> Result<()> {
        let len = self.rope.len_chars();
        if char_idx > len {
            return Err(CoreError::IndexOutOfBounds(char_idx, len));
        }
        self.rope.insert(char_idx, text);
        Ok(())
    }

    /// Remove um intervalo de caracteres entre `start_char_idx` (inclusivo) e `end_char_idx` (exclusivo).
    ///
    /// # Erros
    /// Retorna `CoreError::IndexOutOfBounds` se os limites forem inválidos ou `start > end`.
    pub fn remove(&mut self, start_char_idx: usize, end_char_idx: usize) -> Result<()> {
        let len = self.rope.len_chars();
        if start_char_idx > end_char_idx || end_char_idx > len {
            return Err(CoreError::IndexOutOfBounds(end_char_idx, len));
        }
        if start_char_idx == end_char_idx {
            return Ok(());
        }
        self.rope.remove(start_char_idx..end_char_idx);
        Ok(())
    }

    /// Obtém uma linha específica como String (0-indexada).
    pub fn line(&self, line_idx: usize) -> Option<String> {
        if line_idx >= self.rope.len_lines() {
            return None;
        }
        let line_slice = self.rope.line(line_idx);
        Some(line_slice.to_string())
    }

    /// Extrai uma fatia de texto entre os caracteres `start` e `end`.
    pub fn slice(&self, start: usize, end: usize) -> Option<String> {
        let len = self.rope.len_chars();
        if start > end || end > len {
            return None;
        }
        let slice = self.rope.slice(start..end);
        Some(slice.to_string())
    }

    /// Converte o buffer inteiro em uma `String`.
    pub fn to_string_content(&self) -> String {
        self.rope.to_string()
    }
}

impl fmt::Debug for NoteBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NoteBuffer")
            .field("chars", &self.len_chars())
            .field("lines", &self.len_lines())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_operations() -> Result<()> {
        let mut buf = NoteBuffer::from_str("Olá, Mundo!");
        assert_eq!(buf.len_chars(), 11);
        assert_eq!(buf.len_lines(), 1);

        // Inserção no meio
        buf.insert(4, "bravo ")?;
        assert_eq!(buf.to_string_content(), "Olá, bravo Mundo!");

        // Remoção
        buf.remove(4, 10)?;
        assert_eq!(buf.to_string_content(), "Olá, Mundo!");

        // Fatiamento
        let slice = match buf.slice(0, 3) {
            Some(s) => s,
            None => return Err(CoreError::IndexOutOfBounds(3, buf.len_chars())),
        };
        assert_eq!(slice, "Olá");

        // Validação de erro fora dos limites
        assert!(buf.insert(999, "teste").is_err());
        assert!(buf.remove(50, 60).is_err());

        Ok(())
    }

    #[test]
    fn test_high_volume_typing_simulation() -> Result<()> {
        let mut buf = NoteBuffer::empty();

        // Simula a digitação contínua de 5.000 palavras em loop
        for i in 0..5000 {
            buf.insert(buf.len_chars(), "palavra ")?;
            if i % 500 == 0 {
                // Inserção no início
                buf.insert(0, "topo\n")?;
            }
        }

        assert!(buf.len_chars() > 30000);
        assert!(buf.len_lines() > 5);

        Ok(())
    }
}
