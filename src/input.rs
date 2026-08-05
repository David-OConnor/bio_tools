//! A descriptor of CLI input arguments to a program. These are applicable to both GUis,
//! and APIs.

#[derive(Clone, Copy, PartialEq)]
pub enum FieldType {
    /// todo: Replace these specific types with Text fields, A/R.
    DnaSequence,
    RnaSequence,
    AminoAcidSequence,
    Smiles,
    PositiveInteger,
    NonNegativeInteger,
    Integer,
    Text,
}

// todo: Repetative with FieldType. figure out how you use them. They may both have uses.
#[derive(Clone, PartialEq)]
pub enum Field {
    /// todo: we could represent these as na_seq types, but they may include wildcards etc.
    /// todo: Would take some thought; text is fine for now.
    DnaSequence(String),
    RnaSequence(String),
    AminoAcidSequence(String),
    Smiles(String),
    PositiveInteger(u32),
    NonNegativeInteger(u32),
    Integer(i32),
    Text(String),
}

impl Field {
    pub fn get_type(&self) -> FieldType {
        use FieldType::*;
        match self {
            Field::DnaSequence(_) => DnaSequence,
            Field::RnaSequence(_) => RnaSequence,
            Field::AminoAcidSequence(_) => AminoAcidSequence,
            Field::Smiles(_) => Smiles,
            Field::PositiveInteger(_) => Integer,
            Field::NonNegativeInteger(_) => Integer,
            Field::Integer(_) => Integer,
            Field::Text(_) => Text,
        }
    }
}

#[derive(Clone, Debug)]
pub struct InputField {
    // pub field_type: FieldType,
    pub data: Field,
}
