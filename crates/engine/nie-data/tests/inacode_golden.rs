#![allow(clippy::pedantic)]
//! Tests golden `inacode` — valeurs RÉELLES des 10 listes RDBN, tirées de :
//! `data/common/gamedata/inacode/inacode_config_1.01.57.00.cfg.bin` (VFS Steam),
//! décodé RDBN par `nie-formats` (`cfgbin::read_values`) en forme iecode
//! `{ lists: [ { name, typeName, values } ] }`.
//!
//! Port 1:1 d'inagle `packages/inagle/src/parsers/inacode-config.ts` (switch `list.name`),
//! étendu aux 10 listes réelles. Vérité terrain extraite via
//! un exemple jetable, non versionné.
//! La fixture embarque un échantillon représentatif (2-3 entrées par liste, salons complets).

use nie_data::hash::HashId;
use nie_data::inacode::{InacodeRange, parse_inacode_config};
use serde_json::{Value, json};

/// Fixture = vrais noeuds du dump, forme iecode `{lists:[{name,typeName,values}]}`.
fn fixture() -> Value {
    json!({
        "version": 100,
        "lists": [
            {
                "name": "m_InacodeStampDataList",
                "typeName": "INACODE_STAMP_DATA",
                "values": [
                    { "idCrc": "0x027864BF", "imgNameCrc": "0xBC6BEC78", "imgPathCrc": "0xACEC7AD6" },
                    { "idCrc": "0x2955377C", "imgNameCrc": "0x2562BDC2", "imgPathCrc": "0x2A780878" },
                    { "idCrc": "0x304E063D", "imgNameCrc": "0x52658D54", "imgPathCrc": "0xE124DBDD" }
                ]
            },
            {
                "name": "m_InacodeAuthorNameDataList",
                "typeName": "INACODE_AUTHOR_NAME_DATA",
                "values": [
                    { "authorCrc": "0x1F6CD8E8", "authorNameTextIdCrc": "0x8B9D5824" },
                    { "authorCrc": "0x4346BBBD", "authorNameTextIdCrc": "0xD7B73B71" },
                    { "authorCrc": "0x5A5D8AFC", "authorNameTextIdCrc": "0xCEAC0A30" }
                ]
            },
            {
                "name": "m_InacodePhotoImgDataList",
                "typeName": "INACODE_PHOTO_IMG_DATA",
                "values": [
                    { "idCrc": "0x9B7AC167", "imgNameCrc": "0x9D1B046B", "imgPathCrc": "0x1CC1EE90" },
                    { "idCrc": "0xB05792A4", "imgNameCrc": "0x041255D1", "imgPathCrc": "0x9A559C3E" },
                    { "idCrc": "0xFF160463", "imgNameCrc": "0x9A76C072", "imgPathCrc": "0x8750AC86" }
                ]
            },
            {
                "name": "m_InacodeMemberDataList",
                "typeName": "INACODE_MEMBER_DATA",
                "values": [
                    { "idCrc": "0x1F6CD8E8", "memberCond": "0xFFFFFFFF" },
                    { "idCrc": "0x4346BBBD", "memberCond": "0xFFFFFFFF" },
                    { "idCrc": "0x0C072D7A", "memberCond": "0xFFFFFFFF" }
                ]
            },
            {
                "name": "m_InacodeCommentSetIdDataList",
                "typeName": "INACODE_COMMENT_SET_ID_DATA",
                "values": [
                    { "idCrc": "0x1985386D" },
                    { "idCrc": "0x808C69D7" },
                    { "idCrc": "0xF78B5941" }
                ]
            },
            {
                "name": "m_InacodeRoomDataList",
                "typeName": "INACODE_ROOM_DATA",
                "values": [
                    {
                        "idCrc": "0xF3E5318D", "orderby": 1, "name": "0x8E6647BE",
                        "openBitFlag": "0x00000000", "newBitFlag": "0x00000000",
                        "newCommentBitFlag": "0x00000000", "readedCommentFlagIndex": 1,
                        "openCond": "AAAAAA8FNbkZNtoAAQAyAAAng3E=",
                        "member": [0, 28], "comment": [0, 68]
                    },
                    {
                        "idCrc": "0x635A2C1C", "orderby": 4, "name": "0x7FDCB9FD",
                        "openBitFlag": "0x00000000", "newBitFlag": "0x00000000",
                        "newCommentBitFlag": "0x00000000", "readedCommentFlagIndex": 4,
                        "openCond": "AAAAAA8FNbkZNtoAAQAyAACcfHE=",
                        "member": [28, 5], "comment": [68, 6]
                    }
                ]
            },
            {
                "name": "m_InacodeSelectionDataList",
                "typeName": "INACODE_SELECTION_DATA",
                "values": [
                    { "index": 1, "text": "0x481EDC75" },
                    { "index": 2, "text": "0xD1178DCF" },
                    { "index": 1, "text": "0xC94AF361" }
                ]
            },
            {
                "name": "m_InacodeMentionDataList",
                "typeName": "INACODE_MENTION_DATA",
                "values": [
                    { "charaIdCrc": "0x272A7EB9" },
                    { "charaIdCrc": "0x1F6CD8E8" },
                    { "charaIdCrc": "0x272A7EB9" }
                ]
            },
            {
                "name": "m_InacodeCommentDataList",
                "typeName": "INACODE_COMMENT_DATA",
                "values": [
                    {
                        "author": "0x272A7EB9", "overwriteAuthor": "0x00000000", "text": "0xA1246860",
                        "isMentionAll": false, "mention": [0, 0], "hour": 16, "minutes": 40,
                        "selectionFlagIndex": 0, "selection": [0, 0], "disableReplyCond": "0xFFFFFFFF",
                        "stamp": "0x00000000", "photoImg": "0x00000000", "needSeparatorLine": true
                    },
                    {
                        "author": "0x272A7EB9", "overwriteAuthor": "0x00000000", "text": "0x8A093BA3",
                        "isMentionAll": true, "mention": [0, 0], "hour": 16, "minutes": 40,
                        "selectionFlagIndex": 1, "selection": [0, 2],
                        "disableReplyCond": "AAAAAA8FNbkZNtoAAQAyAAAnLnE=",
                        "stamp": "0x00000000", "photoImg": "0x00000000", "needSeparatorLine": false
                    }
                ]
            },
            {
                "name": "m_InacodeCommentSetDataList",
                "typeName": "INACODE_COMMENT_SET_DATA",
                "values": [
                    { "idCrc": "0x2E5BC85F", "openCond": "0xFFFFFFFF", "openedFlagIndex": 1, "comment": [0, 3] },
                    {
                        "idCrc": "0xB75299E5",
                        "openCond": "AAAAABgFNZkhwUAACgEoAAYCMgAAAAEyAAAAAXg=",
                        "openedFlagIndex": 2, "comment": [3, 3]
                    },
                    {
                        "idCrc": "0xC055A973",
                        "openCond": "AAAAABgFNZkhwUAACgEoAAYCMgAAAAEyAAAAAng=",
                        "openedFlagIndex": 3, "comment": [6, 4]
                    }
                ]
            }
        ]
    })
}

#[test]
fn stamps_reels() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.stamps.len(), 3);
    let s0 = &cfg.stamps[0];
    assert_eq!(s0.id_crc, HashId(0x0278_64BF));
    assert_eq!(s0.img_name_crc, HashId(0xBC6B_EC78));
    assert_eq!(s0.img_path_crc, HashId(0xACEC_7AD6));
    assert_eq!(cfg.stamps[2].id_crc.to_hex(), "0x304E063D");
    // find_stamp (port byStampId d'inagle).
    assert_eq!(cfg.find_stamp(HashId(0x2955_377C)), Some(&cfg.stamps[1]));
    assert_eq!(cfg.find_stamp(HashId(0xDEAD_BEEF)), None);
}

#[test]
fn authors_reels() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.authors.len(), 3);
    assert_eq!(cfg.authors[0].author_crc, HashId(0x1F6C_D8E8));
    assert_eq!(cfg.authors[0].author_name_text_id_crc, HashId(0x8B9D_5824));
    assert_eq!(cfg.authors[2].author_crc.to_hex(), "0x5A5D8AFC");
}

#[test]
fn photo_imgs_reels() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.photo_imgs.len(), 3);
    assert_eq!(cfg.photo_imgs[0].id_crc, HashId(0x9B7A_C167));
    assert_eq!(cfg.photo_imgs[0].img_name_crc, HashId(0x9D1B_046B));
    assert_eq!(cfg.photo_imgs[1].img_path_crc, HashId(0x9A55_9C3E));
}

#[test]
fn members_reels_et_condition_brute() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.members.len(), 3);
    assert_eq!(cfg.members[0].id_crc, HashId(0x1F6C_D8E8));
    // memberCond = repli "0xFFFFFFFF" (offset Condition non résoluble) conservé brut.
    assert_eq!(cfg.members[0].member_cond, "0xFFFFFFFF");
    assert_eq!(cfg.members[2].id_crc.to_hex(), "0x0C072D7A");
}

#[test]
fn comment_set_ids_reels() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.comment_set_ids.len(), 3);
    assert_eq!(cfg.comment_set_ids[0].id_crc, HashId(0x1985_386D));
    assert_eq!(cfg.comment_set_ids[2].id_crc.to_hex(), "0xF78B5941");
}

#[test]
fn rooms_reels_complets() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.rooms.len(), 2);
    let r0 = &cfg.rooms[0];
    assert_eq!(r0.id_crc, HashId(0xF3E5_318D));
    assert_eq!(r0.orderby, 1);
    assert_eq!(r0.name, HashId(0x8E66_47BE));
    assert_eq!(r0.open_bit_flag, HashId::ZERO);
    assert_eq!(r0.readed_comment_flag_index, 1);
    // openCond = condition base64 (offset Condition résolu) conservée brute.
    assert_eq!(r0.open_cond, "AAAAAA8FNbkZNtoAAQAyAAAng3E=");
    // ShortTuple → plages (offset, count).
    assert_eq!(
        r0.member,
        InacodeRange {
            offset: 0,
            count: 28
        }
    );
    assert_eq!(
        r0.comment,
        InacodeRange {
            offset: 0,
            count: 68
        }
    );
    let r1 = &cfg.rooms[1];
    assert_eq!(r1.orderby, 4);
    assert_eq!(
        r1.member,
        InacodeRange {
            offset: 28,
            count: 5
        }
    );
    assert_eq!(
        r1.comment,
        InacodeRange {
            offset: 68,
            count: 6
        }
    );
}

#[test]
fn selections_reels() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.selections.len(), 3);
    assert_eq!(cfg.selections[0].index, 1);
    assert_eq!(cfg.selections[0].text, HashId(0x481E_DC75));
    assert_eq!(cfg.selections[1].index, 2);
    assert_eq!(cfg.selections[2].text.to_hex(), "0xC94AF361");
}

#[test]
fn mentions_reels() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.mentions.len(), 3);
    assert_eq!(cfg.mentions[0].chara_id_crc, HashId(0x272A_7EB9));
    assert_eq!(cfg.mentions[1].chara_id_crc, HashId(0x1F6C_D8E8));
}

#[test]
fn comments_reels_complets() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.comments.len(), 2);
    let c0 = &cfg.comments[0];
    assert_eq!(c0.author, HashId(0x272A_7EB9));
    assert_eq!(c0.overwrite_author, HashId::ZERO);
    assert_eq!(c0.text, HashId(0xA124_6860));
    assert!(!c0.is_mention_all);
    assert_eq!(
        c0.mention,
        InacodeRange {
            offset: 0,
            count: 0
        }
    );
    assert_eq!(c0.hour, 16);
    assert_eq!(c0.minutes, 40);
    assert_eq!(c0.selection_flag_index, 0);
    assert_eq!(c0.disable_reply_cond, "0xFFFFFFFF");
    assert_eq!(c0.stamp, HashId::ZERO);
    assert_eq!(c0.photo_img, HashId::ZERO);
    assert!(c0.need_separator_line);
    // c1 : isMentionAll=true, selection=[0,2] vers m_InacodeSelectionDataList, openCond base64.
    let c1 = &cfg.comments[1];
    assert!(c1.is_mention_all);
    assert_eq!(c1.text, HashId(0x8A09_3BA3));
    assert_eq!(
        c1.selection,
        InacodeRange {
            offset: 0,
            count: 2
        }
    );
    assert_eq!(c1.disable_reply_cond, "AAAAAA8FNbkZNtoAAQAyAAAnLnE=");
    assert!(!c1.need_separator_line);
}

#[test]
fn comment_sets_reels() {
    let cfg = parse_inacode_config(&fixture());
    assert_eq!(cfg.comment_sets.len(), 3);
    let cs0 = &cfg.comment_sets[0];
    assert_eq!(cs0.id_crc, HashId(0x2E5B_C85F));
    assert_eq!(cs0.open_cond, "0xFFFFFFFF");
    assert_eq!(cs0.opened_flag_index, 1);
    assert_eq!(
        cs0.comment,
        InacodeRange {
            offset: 0,
            count: 3
        }
    );
    assert_eq!(
        cfg.comment_sets[1].open_cond,
        "AAAAABgFNZkhwUAACgEoAAYCMgAAAAEyAAAAAXg="
    );
    assert_eq!(
        cfg.comment_sets[2].comment,
        InacodeRange {
            offset: 6,
            count: 4
        }
    );
}

#[test]
fn references_croisees_internes() {
    // Garde anti-régression : les CRC se recoupent entre listes (vérité terrain), prouvant
    // que le décodage Hash (signé→non-signé) est cohérent partout.
    let cfg = parse_inacode_config(&fixture());
    // member[0].idCrc == author[0].authorCrc (le membre EST un auteur).
    assert_eq!(cfg.members[0].id_crc, cfg.authors[0].author_crc);
    // comment[0].author == mention[0].charaIdCrc (l'auteur du message est mentionnable).
    assert_eq!(cfg.comments[0].author, cfg.mentions[0].chara_id_crc);
    // room0.member [0,28] + room1.member [28,5] couvrent les 33 m_InacodeMemberDataList réels.
    let m0 = cfg.rooms[0].member;
    let m1 = cfg.rooms[1].member;
    assert_eq!(
        i32::from(m0.offset) + i32::from(m0.count),
        i32::from(m1.offset)
    );
    assert_eq!(i32::from(m1.offset) + i32::from(m1.count), 33);
    // room0.comment [0,68] + room1.comment [68,6] couvrent les 74 m_InacodeCommentSetIdDataList.
    let cm1 = cfg.rooms[1].comment;
    assert_eq!(i32::from(cm1.offset) + i32::from(cm1.count), 74);
}

#[test]
fn lists_absentes_donnent_vecs_vides() {
    // Robustesse : un root sans la moindre liste inacode → config par défaut (tout vide).
    let cfg = parse_inacode_config(&json!({ "lists": [] }));
    assert!(cfg.stamps.is_empty());
    assert!(cfg.comments.is_empty());
    assert!(cfg.rooms.is_empty());
}
