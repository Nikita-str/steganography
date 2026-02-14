use std::borrow::Cow;
use std::str::Chars;
use rand::rngs::ThreadRng;
use rand::Rng;

use crate::reader::ConstBytesReader;
use crate::writer::IterByteWriter;
use crate::{Error, Result};

pub mod s3;
pub mod id;
pub mod num;
pub mod time;
pub mod price;
pub mod txt_enum;
// TODO: mail (especially temp mails -- the existence of a mail in tmp-mail domains is stored for a short time)

pub mod str_writer;

// TODO: code (rust, python)
// pub mod csv;

// TODO: exchange [a -> o; i -> e;]
// TODO: chunked repeat wo key (count amount of repeatness in chunk % 2)

pub trait RepeatTypo {
    fn next_typo(&mut self, cur: char, next: char) -> char;
}
pub struct RepeatConstTypo {
    a: char,
    b: char,
}
impl RepeatConstTypo {
    /// # panic
    /// * if `a == b`
    pub fn new(a: char, b: char) -> Self {
        assert_ne!(a, b);
        Self { a, b }
    }
}
impl RepeatTypo for RepeatConstTypo {
    fn next_typo(&mut self, cur: char, next: char) -> char {
        if cur == next  { 
            if self.a == next {
                self.b
            } else {
                self.a
            }
        } else { 
            cur 
        }
    }
}

pub struct RepeatCharHider<'s, Typo> {
    pub initial: Cow<'s, str>,
    /// should be >= 5 
    pub bit_freq: usize,
    pub msg: Cow<'s, [u8]>,
    pub typo: Typo,
}
impl<'s, Typo: RepeatTypo> RepeatCharHider<'s, Typo> {
    pub fn new_ref(initial: &'s str, msg: &'s [u8], bit_freq: usize, typo: Typo) -> Self {
        Self {
            initial: Cow::Borrowed(initial),
            bit_freq,
            msg: Cow::Borrowed(msg),
            typo,
        }
    }
    pub fn new(initial: String, msg: &'s [u8], bit_freq: usize, typo: Typo) -> Self {
        Self {
            initial: Cow::Owned(initial),
            bit_freq,
            msg: Cow::Borrowed(msg),
            typo,
        }
    }

    fn skip(mut iter: impl Iterator<Item = char>, n: usize, s: &mut String) -> bool {
        for _ in 0..n {
            if let Some(c) = iter.next() {
                s.push(c);
            } else {
                return false
            }
        }
        return true
    }

    fn repeat_char(ret: &mut String, typo: &mut Typo, char: char, next_char: char) {
        ret.push(char);
        ret.push(typo.next_typo(char, next_char));
        ret.push(next_char);
    }

    fn write_bit_1(ret: &mut String, rng: &mut ThreadRng, mut char_iter: &mut Chars, typo: &mut Typo, bit_freq: usize) -> bool {
        let skip = rng.random_range(0..(bit_freq - 1));
        Self::skip(&mut char_iter, skip, ret);
        
        let (Some(char), Some(next_char)) = (char_iter.next(), char_iter.next()) else {
            return true
        };
        Self::repeat_char(ret, typo, char, next_char);

        let skip = bit_freq - skip - 1; // 2;
        if !Self::skip(&mut char_iter, skip, ret) {
            return true
        }

        false
    }

    pub fn hide(mut self) -> Result<String> {
        let mut ret = String::with_capacity(self.initial.len() + self.msg.len() * 8);
        let mut rng = rand::rng();
        let rng = &mut rng;

        let mut char_iter = self.initial.chars();

        let typo = &mut self.typo;

        let mut bit_reader = IterByteWriter::new(self.msg.iter().copied(), 1);
        macro_rules! err_not_enough_size {
            () => {
                Err(Error::NotEnoughSizeOfInit(
                    bit_reader.take_iter().count() + 1
                ))
            };
        }

        loop {
            if bit_reader.is_done() { break }

            let mut bit = false;
            bit_reader.write_bits(|x|{
                bit = x > 0;
                true
            });

            if bit {
                if Self::write_bit_1(&mut ret, rng, &mut char_iter, typo, self.bit_freq) {
                    return err_not_enough_size!()
                }
            } else {
                if !Self::skip(&mut char_iter, self.bit_freq, &mut ret) {
                    return err_not_enough_size!()
                }
            }
        }
        // on the last chunk we have 2 typo
        Self::write_bit_1(&mut ret, rng, &mut char_iter, typo, self.bit_freq - 3);
        if let (Some(char), Some(next_char)) = (char_iter.next(), char_iter.next()) {
            Self::repeat_char(&mut ret, typo, char, next_char);
        }

        // push others chars (without errors? we already know that text is ended, so we can make some errors)
        char_iter.for_each(|c|ret.push(c));

        Ok(ret)
    }
}

pub struct RepeatCharRevealer<'s> {
    pub initial: Cow<'s, str>,
    pub modified: Cow<'s, str>,
    /// should be >= 5 
    pub bit_freq: usize,
    pub with_header: bool, // TODO: if true => header contains bit_freq & ?len?
}
impl<'s> RepeatCharRevealer<'s> {
    pub fn reveal(self) -> Result<Vec<u8>> {
        let mut ret = Vec::new();
        let mut reader = ConstBytesReader::new(1);

        let mut init_iter = self.initial.chars();
        let mut mod_iter = self.modified.chars();

        'one_bit_chunk: loop {
            macro_rules! take_next_and_handle_err {
                ($a: ident, $b: ident) => {
                    let Some($b) = mod_iter.next() else { break 'one_bit_chunk };
                    let Some($a) = init_iter.next() else {
                        return Err(Error::InconsistentInitText)
                    };
                };
            }

            let mut chunk_index = 0;
            let mut bit = 0;

            for _ in 0..self.bit_freq {
                take_next_and_handle_err!(a, b);
                if a != b {
                    mod_iter.next();
                    bit = 1;
                    break;
                }
                chunk_index += 1;
            }

            // stay on pos before potential duplicate that signed the end
            for _ in chunk_index..self.bit_freq {
                take_next_and_handle_err!(a, b);
                if a != b { break 'one_bit_chunk }
            }

            if let Some(byte) = reader.try_take_next_le_byte(bit) {
                ret.push(byte);
            }
        }
        assert!(reader.is_not_started());

        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INIT: &str = "\
    Наказанный сынок не успел подрасти\n\
    Капризное весло отказалось грести\n\
    Упрямый парашют не раскрылся в свой срок\n\
    А залётный бумеранг посмел поверить в то, что мол —\n\
    обратной дороги нет\n\
    обратной дороги нет\n\
    обратной дороги нет\n\
    обратной дороги нет\n\
    нет уж лучше ты послушай как впивается в ладони дождь\n\
    слушай как по горлу пробегает мышь\n\
    слушай как под сердцем возникает брешь\n\
    как в желудке копошится зима\n\
    как ползёт по позвоночнику землистый лишай\n\
    как вливается в глазницы родниковый потоп\n\
    как настырный одуванчик раздирает асфальт\n\
    как ржавеют втихомолку потаённые прозрачные двери\n\
    слушай как сквозь кожу прорастает рожь\n\
    слушай как по горлу пробегает мышь\n\
    слушай как в желудке пузырится смех\n\
    слушай как спешит по гулким венам вдаль твоя\n\
    сладкая радуга...\n\
    звонкая радуга...\n\
    как на яблоне на ветке созревает звезда\n\
    крошечная поздняя милая ручная...\n\
    слушай как блуждают по покинутым селениям\n\
    шальные хороводы деревянных невест...
    \n\
    слушай как под сердцем колосится рожь\n\
    слушай как по горлу пробегает мышь\n\
    слушай как в желудке распухает ночь\n\
    как вонзается в ладонь стебелёк
    \n\
    как лениво высыхает молоко на губах\n\
    как ворочается в печени червивый клубок\n\
    как шевелятся кузнечики в густом янтаре\n\
    погружаясь в изнурительное бегство в никуда из ниоткуда...
    \n\
    вот и хорошо, вот и баиньки\n\
    страшно безымянному заиньке\n\
    под глазастыми заборами в удушливых потёмках\n\
    своего замысловатого сырого нутра...\
    ";

    #[test]
    fn test_repeat_char_hide_reveal() -> Result<()> {
        let init_msg_pairs = [
            (INIT.to_owned(), "нет.", 23),
            (INIT.repeat(17 * 8 * 2 + 1), INIT, 17),
            (". ".repeat(2000), "прыг-скок", 5),
            (" ".repeat(900), "прыг-скок", 6),
            (" ".repeat(900 - 10), "прыг-скок", 6),
            (" ".repeat(900 - 12), "прыг-скок", 6),
            (" ".repeat(900 - 14), "прыг-скок", 6),
            (" ".repeat(900 - 15), "прыг-скок", 6),
            (".".repeat(2000), "прыг+скок", 7),
            ("x".repeat(16_500), "вот и хорошо, вот и баиньки", 42),
        ];

        for (init, msg, bit_freq) in init_msg_pairs {        
            let typo = RepeatConstTypo::new('.', ' ');
            let hider = RepeatCharHider::new_ref(&init, msg.as_bytes(), bit_freq, typo);
            
            let output = hider.hide()?;
            
            let revealer = RepeatCharRevealer {
                initial: Cow::Borrowed(&init),
                modified: Cow::Borrowed(&output),
                bit_freq: bit_freq,
                with_header: false,
            };
            
            let msg_get = revealer.reveal()?;
            let msg_get = String::from_utf8(msg_get).unwrap();
            assert_eq!(msg_get, msg);
        }

        Ok(())
    }
}