use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use combine::{
    error::UnexpectedParse,
    parser::{
        byte::{bytes_cmp, digit, spaces},
        choice::{choice, optional},
        combinator::r#try,
        item::{item, none_of, one_of},
        range::recognize,
        repeat::{skip_many, skip_many1},
    },
    Parser,
};

pub fn date_time<'a>() -> impl Parser<Input = &'a [u8], Output = DateTime<FixedOffset>> {
    (
        optional((day_of_week(), item(b','))),
        date(),
        time(),
        optional(cfws()),
    )
        .and_then(|(dow, date, (time, tz), _)| {
            if dow.map_or(true, |(dow, _)| date.weekday() == dow) {
                let naive_dt = NaiveDateTime::new(date, time);
                Ok(DateTime::from_utc(naive_dt - tz, tz))
            } else {
                Err(UnexpectedParse::Unexpected)
            }
        })
}

macro_rules! choice_literal {
    ($($s:expr => $v:expr,)+) => {
        choice((
            $(r#try(bytes_cmp($s, |l, r| l.eq_ignore_ascii_case(&r))).map(|_| $v),)+
        ))
    }
}

fn day_of_week<'a>() -> impl Parser<Input = &'a [u8], Output = Weekday> {
    (optional(cfws()), day_name(), optional(cfws())).map(|(_, day_name, _)| day_name)
}

fn day_name<'a>() -> impl Parser<Input = &'a [u8], Output = Weekday> {
    choice_literal! {
        b"mon" => Weekday::Mon,
        b"tue" => Weekday::Tue,
        b"wed" => Weekday::Wed,
        b"thu" => Weekday::Thu,
        b"fri" => Weekday::Fri,
        b"sat" => Weekday::Sat,
        b"sun" => Weekday::Sun,
    }
}

fn date<'a>() -> impl Parser<Input = &'a [u8], Output = NaiveDate> {
    (
        one_or_two_digits_with_cfws(), // day
        month(),
        year(),
    )
        .map(|(day, month, year)| NaiveDate::from_ymd(year, month, day))
}

fn month<'a>() -> impl Parser<Input = &'a [u8], Output = u32> {
    choice_literal! {
        b"jan" => 1,
        b"feb" => 2,
        b"mar" => 3,
        b"apr" => 4,
        b"may" => 5,
        b"jun" => 6,
        b"jul" => 7,
        b"aug" => 8,
        b"sep" => 9,
        b"oct" => 10,
        b"nov" => 11,
        b"dec" => 12,
    }
}

fn year<'a>() -> impl Parser<Input = &'a [u8], Output = i32> {
    (
        optional(cfws()),
        recognize(skip_many1(digit())),
        optional(cfws()),
    )
        .and_then(|(_, s, _): (_, &[u8], _)| {
            if s.len() < 2 {
                return Err(UnexpectedParse::Unexpected);
            }
            let mut year = s
                .iter()
                .fold(0, |year, digit| year * 10 + i32::from(digit - b'0'));
            if s.len() == 2 {
                if year < 50 {
                    year += 2000;
                } else {
                    year += 1900;
                }
            }
            Ok(year)
        })
}

fn time<'a>() -> impl Parser<Input = &'a [u8], Output = (NaiveTime, FixedOffset)> {
    (time_of_day(), zone())
}

fn time_of_day<'a>() -> impl Parser<Input = &'a [u8], Output = NaiveTime> {
    // We explicitly allow single digit to be used for hour, minute, and second,
    // which is different from what the spec says.
    (
        one_or_two_digits_with_cfws(), // hour
        item(b':'),
        one_or_two_digits_with_cfws(), // minute
        optional((
            item(b':'),
            one_or_two_digits_with_cfws(), // second
        )),
    )
        .and_then(|(hour, _, minute, second)| {
            let (second, milli) = match second.map(|(_, s)| s).unwrap_or(0) {
                sec @ 0...59 => (sec, 0),
                sec => (59, (sec - 59) * 1_000),
            };
            NaiveTime::from_hms_milli_opt(hour, minute, second, milli)
                .ok_or(UnexpectedParse::Unexpected)
        })
}

fn zone<'a>() -> impl Parser<Input = &'a [u8], Output = FixedOffset> {
    choice((
        (
            spaces(),
            one_of(b"+-".iter().cloned()),
            digit(),
            digit(),
            digit(),
            digit(),
        )
            .map(|(_, op, d1, d2, d3, d4)| {
                let hour = atoi(d1) * 10 + atoi(d2);
                let minute = atoi(d3) * 10 + atoi(d4);
                let secs = (hour * 3600 + minute * 60) as i32;
                // We treat -0000 as +0000 here as there is nothing else we can
                // do for that case.
                FixedOffset::east(if op == b'-' { -secs } else { secs })
            }),
        obs_zone(),
    ))
}

fn obs_zone<'a>() -> impl Parser<Input = &'a [u8], Output = FixedOffset> {
    (choice_literal! {
        b"ut" => 0,
        b"gmt" => 0,
        b"est" => -5,
        b"edt" => -4,
        b"cst" => -6,
        b"cdt" => -5,
        b"mst" => -7,
        b"mdt" => -6,
        b"pst" => -8,
        b"pdt" => -7,
    })
    .map(|hour| FixedOffset::east(hour * 3600))
}

fn one_or_two_digits_with_cfws<'a>() -> impl Parser<Input = &'a [u8], Output = u32> {
    (
        optional(cfws()),
        digit(),
        optional(digit()),
        optional(cfws()),
    )
        .map(|(_, d1, d2, _)| {
            if let Some(d2) = d2 {
                atoi(d1) * 10 + atoi(d2)
            } else {
                atoi(d1)
            }
        })
}

fn atoi(a: u8) -> u32 {
    u32::from(a - b'0')
}

fn cfws<'a>() -> impl Parser<Input = &'a [u8], Output = ()> {
    (spaces(), skip_many((comment(), spaces()))).map(|_| ())
}

fn comment<'a>() -> impl Parser<Input = &'a [u8], Output = ()> {
    (
        item(b'('),
        skip_many(none_of(br"()\".iter().cloned())),
        item(b')'),
    )
        .map(|_| ())
}
