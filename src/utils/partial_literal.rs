use crate::{
    OrderedLiterals,
    parse::{ConsumeResult, Consumer},
};

pub fn partial_literal_consumer(OrderedLiterals(literals): OrderedLiterals) -> Consumer {
    let mut literals_iter = literals.into_iter();
    let mut curr = literals_iter.next();

    Consumer(Box::new(move |c| {
        let Some(inner) = curr.take().or_else(|| literals_iter.next()).as_mut() else {
            return ConsumeResult::Unconsumed(c);
        };

        // match inner {
        //     LiteralOrWild::Literal(literal) => {
        //         literal.pop_front();
        //     }
        //     LiteralOrWild::Wild { wild, bounded } => {
        //         if wild == c {
        //             return Ok(ConsumeOutput::Consumed);
        //         }
        //     }
        // }

        todo!()
    }))
}

// pub fn partial_literal_parser(
//     OrderedLiterals(literals): OrderedLiterals,
// ) -> Parser<impl Iterator<Item = ParseResult>> {
//     let mut literals_iter = literals.into_iter();
//     let mut curr = literals_iter.next();

//     Parser(Box::new(move |c| {}))
// }

// pub fn tool_call_trigger_parser(
//     OrderedLiterals(triggers): OrderedLiterals,
// ) -> impl Iterator<Item = ParseResult> {
//     todo!()
// }
