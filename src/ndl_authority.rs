use serde::Deserialize;
use std::fmt::Write;

pub const AUTHORITY_ENDPOINT: &str = "https://id.ndl.go.jp/auth/ndla/sparql";
pub const AUTHORITY_BATCH_SIZE: usize = 50;

/// Build one bounded SPARQL 1.1 query, retaining the caller's author positions.
pub fn build_authority_query(names: &[&str]) -> String {
    let mut query = String::from(
        "PREFIX foaf: <http://xmlns.com/foaf/0.1/>\n\
         PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n\
         SELECT ?slot (SAMPLE(?entity) AS ?id) WHERE {\nVALUES (?slot ?name) {\n",
    );
    for (slot, name) in names.iter().enumerate() {
        let clean = name.trim();
        if clean.is_empty() {
            continue;
        }
        write_name(&mut query, slot, clean);
        let compact = clean.chars().any(char::is_whitespace).then(|| {
            clean
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        });
        if let Some(compact) = &compact {
            write_name(&mut query, slot, compact);
        }
        let mut parts = clean.split_whitespace();
        if let (Some(first), Some(last), None) = (parts.next(), parts.next(), parts.next()) {
            let comma = format!("{first}, {last}");
            if comma != clean && compact.as_deref() != Some(comma.as_str()) {
                write_name(&mut query, slot, &comma);
            }
        }
    }
    query.push_str(
        "}\n{ ?auth foaf:primaryTopic ?entity . ?entity foaf:name ?name . }\n\
         UNION { ?auth foaf:primaryTopic ?entity ; rdfs:label ?name . }\n\
         } GROUP BY ?slot HAVING (COUNT(DISTINCT ?entity) = 1)",
    );
    query
}

fn write_name(query: &mut String, slot: usize, name: &str) {
    write!(query, "({slot} \"").expect("writing to a String cannot fail");
    for character in name.chars() {
        match character {
            '\\' => query.push_str("\\\\"),
            '"' => query.push_str("\\\""),
            '\n' => query.push_str("\\n"),
            '\r' => query.push_str("\\r"),
            '\t' => query.push_str("\\t"),
            c if c.is_control() => {
                write!(query, "\\u{:04X}", c as u32).expect("writing to a String cannot fail");
            }
            c => query.push(c),
        }
    }
    query.push_str("\")\n");
}

/// Extract only NDL name authority identifiers, never another provider's ID.
pub fn entity_id(uri: &str) -> Option<&str> {
    let path = uri
        .strip_prefix("http://id.ndl.go.jp/auth/")
        .or_else(|| uri.strip_prefix("https://id.ndl.go.jp/auth/"))?;
    let id = path
        .strip_prefix("entity/")
        .or_else(|| path.strip_prefix("ndlna/"))?;
    (!id.is_empty() && id.bytes().all(|c| c.is_ascii_digit())).then_some(id)
}

#[derive(Deserialize)]
struct Results {
    results: Bindings,
}

#[derive(Deserialize)]
struct Bindings {
    bindings: Vec<Binding>,
}

#[derive(Deserialize)]
struct Binding {
    slot: Term,
    id: Term,
}

#[derive(Deserialize)]
struct Term {
    #[serde(rename = "type")]
    kind: String,
    value: String,
}

#[derive(Default)]
enum Candidate {
    #[default]
    Missing,
    Resolved(String),
    Ambiguous,
}

/// Ignore ambiguous identities even if a server returns duplicate/conflicting rows.
pub fn parse_authority_results(
    json: &str,
    author_count: usize,
) -> Result<Vec<Option<String>>, String> {
    let results: Results = serde_json::from_str(json)
        .map_err(|error| format!("NDL authority response parse failed: {error}"))?;
    let mut candidates: Vec<Candidate> = (0..author_count).map(|_| Candidate::Missing).collect();
    for binding in results.results.bindings {
        let slot = binding
            .slot
            .value
            .parse::<usize>()
            .map_err(|_| "NDL authority response has an invalid author position".to_string())?;
        let candidate = candidates
            .get_mut(slot)
            .ok_or_else(|| "NDL authority response has an unknown author position".to_string())?;
        if !matches!(binding.slot.kind.as_str(), "literal" | "typed-literal")
            || binding.id.kind != "uri"
        {
            return Err("NDL authority response has an invalid binding type".to_string());
        }
        let id = entity_id(&binding.id.value)
            .ok_or_else(|| "NDL authority response has an invalid authority URI".to_string())?;
        match candidate {
            Candidate::Missing => *candidate = Candidate::Resolved(id.to_string()),
            Candidate::Resolved(previous) if previous != id => *candidate = Candidate::Ambiguous,
            _ => {}
        }
    }
    Ok(candidates
        .into_iter()
        .map(|candidate| match candidate {
            Candidate::Resolved(id) => Some(id),
            Candidate::Missing | Candidate::Ambiguous => None,
        })
        .collect())
}

#[cfg(test)]
mod tests;
