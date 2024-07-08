use biome_graphql_parser::parse_graphql;
use biome_graphql_syntax::GraphqlFragmentDefinition;
use biome_graphql_syntax::GraphqlFragmentSpread;

use crate::semantic_model;
use crate::HasDeclarationAstNode;
use crate::IsBindingAstNode;

use super::assert_nodes_eq;
use super::extract_node;

#[test]
fn ok_fragment_spread() {
    let src = r#"
fragment HeroDetails on Character {
    name
}

query query {
    hero {
        ...HeroDetails
    }
}"#;
    let parse_result = parse_graphql(src);
    let model = semantic_model(&parse_result.tree());
    let bindings = model.all_bindings().collect::<Vec<_>>();

    assert_nodes_eq(bindings, &["HeroDetails", "query"]);

    let unresolved_references = model.all_unresolved_references().collect::<Vec<_>>();
    assert_nodes_eq(unresolved_references, &["Character"]);

    let fragment_definition = extract_node::<GraphqlFragmentDefinition>(&parse_result);
    let fragment_spread = extract_node::<GraphqlFragmentSpread>(&parse_result);
    let fragment_binding = fragment_spread.binding_node(&model).unwrap();

    assert_eq!(fragment_binding, fragment_definition);

    let fragment_reference = fragment_definition.all_reference_nodes(&model);
    assert_eq!(fragment_reference, vec![fragment_spread]);
}

#[test]
fn ok_fragment_variable_reference() {
    let src = r#"
fragment HeroDetails on Character {
    name(startWith: $start)
}
"#;
    let parse_result = parse_graphql(src);
    let model = semantic_model(&parse_result.tree());
    let bindings = model.all_bindings().collect::<Vec<_>>();

    assert_nodes_eq(bindings, &["HeroDetails"]);

    let unresolved_references = model.all_unresolved_references().collect::<Vec<_>>();
    assert_nodes_eq(unresolved_references, &["Character", "start"]);
}
