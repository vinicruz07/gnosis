//! Seletor e catálogo visual de stickers e emojis com busca difusa.
//!
//! Permite pesquisar instantaneamente por emojis universais e ícones temáticos
//! para enriquecer notas, pastas e quadros de conhecimento.

use crate::fuzzy::fuzzy_match;

/// Item de sticker/emoji catalogado no sistema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickerItem {
    pub id: String,
    pub icon: String,
    pub name: String,
    pub category: String,
    pub keywords: String,
}

impl StickerItem {
    pub fn new(
        id: impl Into<String>,
        icon: impl Into<String>,
        name: impl Into<String>,
        category: impl Into<String>,
        keywords: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            name: name.into(),
            category: category.into(),
            keywords: keywords.into(),
        }
    }
}

/// Gerenciador e controlador do catálogo de stickers.
#[derive(Debug, Clone)]
pub struct StickerPickerController {
    catalog: Vec<StickerItem>,
}

impl Default for StickerPickerController {
    fn default() -> Self {
        Self::new()
    }
}

impl StickerPickerController {
    /// Inicializa o controlador com o catálogo temático padrão de alta qualidade.
    pub fn new() -> Self {
        let mut catalog = Vec::new();

        // Categoria: Conhecimento & Mente
        catalog.push(StickerItem::new("brain", "🧠", "Cérebro / Mente", "Mente", "brain cerebro mind neuro psicologia inteligencia memoria"));
        catalog.push(StickerItem::new("lightbulb", "💡", "Ideia / Insight", "Mente", "lightbulb idea insight pensamento criatividade luz"));
        catalog.push(StickerItem::new("open_book", "📖", "Livro Aberto", "Mente", "book read leitura estudo aprendizado"));
        catalog.push(StickerItem::new("books", "📚", "Biblioteca", "Mente", "books biblioteca curso referencia literatura"));
        catalog.push(StickerItem::new("microscope", "🔬", "Microscópio / Ciência", "Mente", "science ciencia pesquisa analise laboratorio"));
        catalog.push(StickerItem::new("telescope", "🔭", "Telescópio / Visão", "Mente", "telescope cosmos visao futuro exploracao"));
        catalog.push(StickerItem::new("memo", "📝", "Anotação", "Mente", "memo note nota rascunho texto escrita"));
        catalog.push(StickerItem::new("puzzle", "🧩", "Quebra-Cabeça", "Mente", "puzzle enigma peca logica solucao"));
        catalog.push(StickerItem::new("target", "🎯", "Alvo / Foco", "Mente", "target meta objetivo foco precisao"));
        catalog.push(StickerItem::new("graduation_cap", "🎓", "Academia / Formação", "Mente", "education universidade graduacao diploma"));
        catalog.push(StickerItem::new("key", "🗝️", "Chave do Saber", "Mente", "key segredo chave acesso misterio"));

        // Categoria: Ação & Projetos
        catalog.push(StickerItem::new("rocket", "🚀", "Foguete / Lançamento", "Projetos", "rocket startup release lancamento velocidade"));
        catalog.push(StickerItem::new("lightning", "⚡", "Raio / Energia", "Projetos", "lightning energia agilidade rapido eletricidade"));
        catalog.push(StickerItem::new("fire", "🔥", "Fogo / Destaque", "Projetos", "fire chama quente importante prioridade urgente"));
        catalog.push(StickerItem::new("gear", "⚙️", "Engrenagem / Config", "Projetos", "gear config engrenagem motor sistema setup"));
        catalog.push(StickerItem::new("tools", "🛠️", "Ferramentas", "Projetos", "tools ferramentas build construcao manutencao"));
        catalog.push(StickerItem::new("laptop", "💻", "Computação / Dev", "Projetos", "computer pc laptop software programacao codigo rust"));
        catalog.push(StickerItem::new("check", "✅", "Concluído", "Projetos", "check done sucesso aprovado task"));
        catalog.push(StickerItem::new("pin", "📌", "Fixado / Alfinete", "Projetos", "pin fixo lembrete destaque"));
        catalog.push(StickerItem::new("warning", "⚠️", "Alerta / Atenção", "Projetos", "warning alerta atencao cuidado perigo"));
        catalog.push(StickerItem::new("package", "📦", "Pacote / Módulo", "Projetos", "package caixa modulo crate biblioteca"));
        catalog.push(StickerItem::new("calendar", "📅", "Calendário / Prazo", "Projetos", "calendar data agenda prazo evento tempo"));

        // Categoria: Natureza & Elementos
        catalog.push(StickerItem::new("herb", "🌿", "Erva / Orgânico", "Natureza", "herb planta verde botanica crescimento vida"));
        catalog.push(StickerItem::new("evergreen", "🌲", "Árvore Perene", "Natureza", "tree evergreen floresta bosque sustentabilidade"));
        catalog.push(StickerItem::new("wave", "🌊", "Onda / Fluxo", "Natureza", "wave agua mar fluxo oceano tide"));
        catalog.push(StickerItem::new("moon", "🌙", "Lua Crescente", "Natureza", "moon lua noite ciclo descanso"));
        catalog.push(StickerItem::new("star", "⭐", "Estrela / Favorito", "Natureza", "star estrela favorito brilho celestial"));
        catalog.push(StickerItem::new("sun", "☀️", "Sol / Clareza", "Natureza", "sun sol dia clareza iluminacao calor"));
        catalog.push(StickerItem::new("mountain", "🏔️", "Montanha / Pico", "Natureza", "mountain montanha cume desafio escalada"));
        catalog.push(StickerItem::new("tornado", "🌪️", "Torvelinho / Vento", "Natureza", "tornado vento boreas tempestade ar"));
        catalog.push(StickerItem::new("blossom", "🌸", "Flor de Cerejeira", "Natureza", "flower flor cerejeira beleza sakura"));
        catalog.push(StickerItem::new("ringed_planet", "🪐", "Planeta / Cosmos", "Natureza", "planet planeta saturno universo espaco"));

        // Categoria: Símbolos & Místico
        catalog.push(StickerItem::new("crystal_ball", "🔮", "Oráculo / Visão", "Místico", "crystal ball oraculo misterio magia adivinhacao"));
        catalog.push(StickerItem::new("shield", "🛡️", "Escudo / Proteção", "Místico", "shield protecao defesa seguranca robustez"));
        catalog.push(StickerItem::new("sword", "⚔️", "Espadas / Combate", "Místico", "swords combate guerra resolucao disputa"));
        catalog.push(StickerItem::new("palette", "🎨", "Arte / Design", "Místico", "art palette paleta cores design estetica"));
        catalog.push(StickerItem::new("hourglass", "⏳", "Ampulheta / Tempo", "Místico", "hourglass tempo relogio ampulheta espera"));
        catalog.push(StickerItem::new("scroll", "📜", "Pergaminho / História", "Místico", "scroll pergaminho historia manuscrito documento"));
        catalog.push(StickerItem::new("gem", "💎", "Gema / Joia", "Místico", "gem diamante joia precioso valioso"));
        catalog.push(StickerItem::new("crown", "👑", "Coroa / Soberano", "Místico", "crown coroa lideranca maestria topo"));
        catalog.push(StickerItem::new("dove", "🕊️", "Pompa / Harmonia", "Místico", "dove pomba paz serenidade harmonia"));
        catalog.push(StickerItem::new("sparkles", "✨", "Centelha / Inspiração", "Místico", "sparkles magia especial novidade brilho"));
        catalog.push(StickerItem::new("eye", "👁️", "Olho da Providência", "Místico", "eye olho gnosis visao consciencia percepcao"));
        catalog.push(StickerItem::new("crab", "🦀", "Ferris / Rust", "Místico", "crab caranguejo ferris rust mascote"));

        Self { catalog }
    }

    /// Retorna todos os itens disponíveis no catálogo.
    pub fn all_items(&self) -> &[StickerItem] {
        &self.catalog
    }

    /// Retorna a lista ordenada de categorias disponíveis.
    pub fn categories(&self) -> Vec<&str> {
        vec!["Todos", "Mente", "Projetos", "Natureza", "Místico"]
    }

    /// Filtra itens por categoria específica (ou todos se for "Todos").
    pub fn filter_by_category(&self, category: &str) -> Vec<StickerItem> {
        if category.eq_ignore_ascii_case("Todos") || category.is_empty() {
            return self.catalog.clone();
        }

        self.catalog
            .iter()
            .filter(|item| item.category.eq_ignore_ascii_case(category))
            .cloned()
            .collect()
    }

    /// Busca difusa por texto em todos os nomes, categorias e palavras-chave.
    /// Retorna os itens ordenados de acordo com a precisão (score decrescente).
    pub fn search(&self, query: &str) -> Vec<StickerItem> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.catalog.clone();
        }

        let mut matches: Vec<(i32, StickerItem)> = self
            .catalog
            .iter()
            .filter_map(|item| {
                let search_corpus = format!(
                    "{} {} {} {}",
                    item.name, item.category, item.keywords, item.id
                );
                fuzzy_match(trimmed, &search_corpus).map(|m| (m.score, item.clone()))
            })
            .collect();

        // Ordena por maior pontuação
        matches.sort_by(|a, b| b.0.cmp(&a.0));

        matches.into_iter().map(|(_, item)| item).collect()
    }

    /// Busca combinada por texto e categoria.
    pub fn search_in_category(&self, query: &str, category: &str) -> Vec<StickerItem> {
        let in_category = self.filter_by_category(category);
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return in_category;
        }

        let mut matches: Vec<(i32, StickerItem)> = in_category
            .into_iter()
            .filter_map(|item| {
                let search_corpus = format!(
                    "{} {} {} {}",
                    item.name, item.category, item.keywords, item.id
                );
                fuzzy_match(trimmed, &search_corpus).map(|m| (m.score, item))
            })
            .collect();

        matches.sort_by(|a, b| b.0.cmp(&a.0));
        matches.into_iter().map(|(_, item)| item).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_population() {
        let controller = StickerPickerController::new();
        assert!(controller.all_items().len() >= 35);
        assert!(controller.categories().contains(&"Mente"));
        assert!(controller.categories().contains(&"Projetos"));
    }

    #[test]
    fn test_fuzzy_search_brain() {
        let controller = StickerPickerController::new();
        let results = controller.search("cerebro");
        assert!(!results.is_empty());
        assert_eq!(results[0].icon, "🧠");

        let results_en = controller.search("brain");
        assert!(!results_en.is_empty());
        assert_eq!(results_en[0].icon, "🧠");
    }

    #[test]
    fn test_fuzzy_search_rust_crab() {
        let controller = StickerPickerController::new();
        let results = controller.search("rust");
        assert!(!results.is_empty());
        assert!(results.iter().any(|s| s.icon == "🦀" || s.icon == "💻"));
    }

    #[test]
    fn test_filter_by_category() {
        let controller = StickerPickerController::new();
        let mente_items = controller.filter_by_category("Mente");
        assert!(!mente_items.is_empty());
        assert!(mente_items.iter().all(|s| s.category == "Mente"));

        let all_items = controller.filter_by_category("Todos");
        assert_eq!(all_items.len(), controller.all_items().len());
    }

    #[test]
    fn test_search_in_category() {
        let controller = StickerPickerController::new();
        let results = controller.search_in_category("foco", "Mente");
        assert!(!results.is_empty());
        assert_eq!(results[0].icon, "🎯");
    }
}
