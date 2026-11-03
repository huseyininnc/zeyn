//! A wide grid: 62 columns over 100,000 rows, to show column virtualization.

use zeyn::prelude::*;
use zeyn::{Cursor, PageRequest};

/// Weekly readings of one sensor.
#[entity(table = "readings")]
pub struct Reading {
    #[key]
    pub id: ReadingId,
    pub sensor: String,
    pub week_01: i64,
    pub week_02: i64,
    pub week_03: i64,
    pub week_04: i64,
    pub week_05: i64,
    pub week_06: i64,
    pub week_07: i64,
    pub week_08: i64,
    pub week_09: i64,
    pub week_10: i64,
    pub week_11: i64,
    pub week_12: i64,
    pub week_13: i64,
    pub week_14: i64,
    pub week_15: i64,
    pub week_16: i64,
    pub week_17: i64,
    pub week_18: i64,
    pub week_19: i64,
    pub week_20: i64,
    pub week_21: i64,
    pub week_22: i64,
    pub week_23: i64,
    pub week_24: i64,
    pub week_25: i64,
    pub week_26: i64,
    pub week_27: i64,
    pub week_28: i64,
    pub week_29: i64,
    pub week_30: i64,
    pub week_31: i64,
    pub week_32: i64,
    pub week_33: i64,
    pub week_34: i64,
    pub week_35: i64,
    pub week_36: i64,
    pub week_37: i64,
    pub week_38: i64,
    pub week_39: i64,
    pub week_40: i64,
    pub week_41: i64,
    pub week_42: i64,
    pub week_43: i64,
    pub week_44: i64,
    pub week_45: i64,
    pub week_46: i64,
    pub week_47: i64,
    pub week_48: i64,
    pub week_49: i64,
    pub week_50: i64,
    pub week_51: i64,
    pub week_52: i64,
    pub week_53: i64,
    pub week_54: i64,
    pub week_55: i64,
    pub week_56: i64,
    pub week_57: i64,
    pub week_58: i64,
    pub week_59: i64,
    pub week_60: i64,
}

pub const READINGS: u64 = 100_000;

/// Reading of sensor `n` in week `w`, always the same for the same input.
fn figure(n: u64, w: u64) -> i64 {
    ((n.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ w.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)) >> 40) as i64
        % 10_000
}

fn reading(n: u64) -> Reading {
    Reading {
        id: ReadingId(n as i64),
        sensor: format!("S-{n:06}"),
        week_01: figure(n, 1),
        week_02: figure(n, 2),
        week_03: figure(n, 3),
        week_04: figure(n, 4),
        week_05: figure(n, 5),
        week_06: figure(n, 6),
        week_07: figure(n, 7),
        week_08: figure(n, 8),
        week_09: figure(n, 9),
        week_10: figure(n, 10),
        week_11: figure(n, 11),
        week_12: figure(n, 12),
        week_13: figure(n, 13),
        week_14: figure(n, 14),
        week_15: figure(n, 15),
        week_16: figure(n, 16),
        week_17: figure(n, 17),
        week_18: figure(n, 18),
        week_19: figure(n, 19),
        week_20: figure(n, 20),
        week_21: figure(n, 21),
        week_22: figure(n, 22),
        week_23: figure(n, 23),
        week_24: figure(n, 24),
        week_25: figure(n, 25),
        week_26: figure(n, 26),
        week_27: figure(n, 27),
        week_28: figure(n, 28),
        week_29: figure(n, 29),
        week_30: figure(n, 30),
        week_31: figure(n, 31),
        week_32: figure(n, 32),
        week_33: figure(n, 33),
        week_34: figure(n, 34),
        week_35: figure(n, 35),
        week_36: figure(n, 36),
        week_37: figure(n, 37),
        week_38: figure(n, 38),
        week_39: figure(n, 39),
        week_40: figure(n, 40),
        week_41: figure(n, 41),
        week_42: figure(n, 42),
        week_43: figure(n, 43),
        week_44: figure(n, 44),
        week_45: figure(n, 45),
        week_46: figure(n, 46),
        week_47: figure(n, 47),
        week_48: figure(n, 48),
        week_49: figure(n, 49),
        week_50: figure(n, 50),
        week_51: figure(n, 51),
        week_52: figure(n, 52),
        week_53: figure(n, 53),
        week_54: figure(n, 54),
        week_55: figure(n, 55),
        week_56: figure(n, 56),
        week_57: figure(n, 57),
        week_58: figure(n, 58),
        week_59: figure(n, 59),
        week_60: figure(n, 60),
    }
}

#[action(permission = "stock.product.read")]
pub async fn search_readings(q: DataQuery) -> Result<Page<Reading>> {
    let offset = match &q.page {
        PageRequest::Offset { offset, .. } => *offset,
        PageRequest::After { cursor, .. } => cursor
            .as_ref()
            .and_then(|c| c.0.parse().ok())
            .unwrap_or(0),
    }
    .min(READINGS);
    let end = (offset + u64::from(q.limit())).min(READINGS);
    let desc = q.sort.first().is_some_and(|s| s.desc);
    let page = Page::new(
        (offset..end)
            .map(|i| reading(if desc { READINGS - i } else { i + 1 }))
            .collect(),
    )
    .total(READINGS);
    Ok(if end < READINGS {
        page.next(Cursor(end.to_string()))
    } else {
        page
    })
}

/// Sixty weeks of readings side by side. Only the columns in view (and a few around them) are in
/// the page; the sensor column stays pinned.
#[screen(route = "/wide", permission = "stock.product.read")]
pub fn wide() -> Screen {
    Screen::new(t!("gallery.wide")).body(
        DataGrid::of(search_readings)
            .mode(DataMode::Server)
            .paging(Paging::virtual_scroll())
            .column(Reading::id)
            .width(90)
            .column(Reading::sensor)
            .no_sort()
            .width(120)
            .pin_start()
            .column(Reading::week_01)
            .no_sort()
            .width(96)
            .column(Reading::week_02)
            .no_sort()
            .width(96)
            .column(Reading::week_03)
            .no_sort()
            .width(96)
            .column(Reading::week_04)
            .no_sort()
            .width(96)
            .column(Reading::week_05)
            .no_sort()
            .width(96)
            .column(Reading::week_06)
            .no_sort()
            .width(96)
            .column(Reading::week_07)
            .no_sort()
            .width(96)
            .column(Reading::week_08)
            .no_sort()
            .width(96)
            .column(Reading::week_09)
            .no_sort()
            .width(96)
            .column(Reading::week_10)
            .no_sort()
            .width(96)
            .column(Reading::week_11)
            .no_sort()
            .width(96)
            .column(Reading::week_12)
            .no_sort()
            .width(96)
            .column(Reading::week_13)
            .no_sort()
            .width(96)
            .column(Reading::week_14)
            .no_sort()
            .width(96)
            .column(Reading::week_15)
            .no_sort()
            .width(96)
            .column(Reading::week_16)
            .no_sort()
            .width(96)
            .column(Reading::week_17)
            .no_sort()
            .width(96)
            .column(Reading::week_18)
            .no_sort()
            .width(96)
            .column(Reading::week_19)
            .no_sort()
            .width(96)
            .column(Reading::week_20)
            .no_sort()
            .width(96)
            .column(Reading::week_21)
            .no_sort()
            .width(96)
            .column(Reading::week_22)
            .no_sort()
            .width(96)
            .column(Reading::week_23)
            .no_sort()
            .width(96)
            .column(Reading::week_24)
            .no_sort()
            .width(96)
            .column(Reading::week_25)
            .no_sort()
            .width(96)
            .column(Reading::week_26)
            .no_sort()
            .width(96)
            .column(Reading::week_27)
            .no_sort()
            .width(96)
            .column(Reading::week_28)
            .no_sort()
            .width(96)
            .column(Reading::week_29)
            .no_sort()
            .width(96)
            .column(Reading::week_30)
            .no_sort()
            .width(96)
            .column(Reading::week_31)
            .no_sort()
            .width(96)
            .column(Reading::week_32)
            .no_sort()
            .width(96)
            .column(Reading::week_33)
            .no_sort()
            .width(96)
            .column(Reading::week_34)
            .no_sort()
            .width(96)
            .column(Reading::week_35)
            .no_sort()
            .width(96)
            .column(Reading::week_36)
            .no_sort()
            .width(96)
            .column(Reading::week_37)
            .no_sort()
            .width(96)
            .column(Reading::week_38)
            .no_sort()
            .width(96)
            .column(Reading::week_39)
            .no_sort()
            .width(96)
            .column(Reading::week_40)
            .no_sort()
            .width(96)
            .column(Reading::week_41)
            .no_sort()
            .width(96)
            .column(Reading::week_42)
            .no_sort()
            .width(96)
            .column(Reading::week_43)
            .no_sort()
            .width(96)
            .column(Reading::week_44)
            .no_sort()
            .width(96)
            .column(Reading::week_45)
            .no_sort()
            .width(96)
            .column(Reading::week_46)
            .no_sort()
            .width(96)
            .column(Reading::week_47)
            .no_sort()
            .width(96)
            .column(Reading::week_48)
            .no_sort()
            .width(96)
            .column(Reading::week_49)
            .no_sort()
            .width(96)
            .column(Reading::week_50)
            .no_sort()
            .width(96)
            .column(Reading::week_51)
            .no_sort()
            .width(96)
            .column(Reading::week_52)
            .no_sort()
            .width(96)
            .column(Reading::week_53)
            .no_sort()
            .width(96)
            .column(Reading::week_54)
            .no_sort()
            .width(96)
            .column(Reading::week_55)
            .no_sort()
            .width(96)
            .column(Reading::week_56)
            .no_sort()
            .width(96)
            .column(Reading::week_57)
            .no_sort()
            .width(96)
            .column(Reading::week_58)
            .no_sort()
            .width(96)
            .column(Reading::week_59)
            .no_sort()
            .width(96)
            .column(Reading::week_60)
            .no_sort()
            .width(96),
    )
}
