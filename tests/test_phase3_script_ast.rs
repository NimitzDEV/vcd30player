use std::path::Path;
use vcd30_player::assets::chm::CompHtmlDoc;
use vcd30_player::core::script_ast::{BinOp, CondOp, Expr, ScriptProgram, Statement};

#[test]
fn test_parse_simple_statements() {
    let code = r#"
20 S = -10 : W = 0 : H = 0
40 DRAWCURSOR 272,123
50 CALL IRKEY(X)
55 IF X = 31 THEN GOTO 100
105 DRAWIMAGE "W_MAN.YBM",30,109,0
120 GOTO 200
9000 GOSUB 4000+A
9020 RETURN
95 END
"#;
    let prog = ScriptProgram::parse(code);
    assert_eq!(prog.lines.len(), 9);

    // Line 20: 3 statements
    let line20 = &prog.lines[&20];
    assert_eq!(line20.len(), 3);
    assert_eq!(
        line20[0],
        Statement::Assign(
            b'S',
            Expr::Binary(
                Box::new(Expr::Const(0)),
                BinOp::Sub,
                Box::new(Expr::Const(10))
            )
        )
    );
    assert_eq!(line20[1], Statement::Assign(b'W', Expr::Const(0)));
    assert_eq!(line20[2], Statement::Assign(b'H', Expr::Const(0)));

    // Line 40: DRAWCURSOR
    let line40 = &prog.lines[&40];
    assert_eq!(
        line40[0],
        Statement::DrawCursor(Expr::Const(272), Expr::Const(123))
    );

    // Line 50: CALL IRKEY(X)
    let line50 = &prog.lines[&50];
    assert_eq!(line50[0], Statement::CallIrkey(b'X'));

    // Line 55: IF X = 31 THEN GOTO 100
    let line55 = &prog.lines[&55];
    assert_eq!(
        line55[0],
        Statement::IfThen {
            lhs: Expr::Var(b'X'),
            op: CondOp::Eq,
            rhs: Expr::Const(31),
            stmt: Box::new(Statement::Goto(Expr::Const(100)))
        }
    );

    // Line 105: DRAWIMAGE
    let line105 = &prog.lines[&105];
    assert_eq!(
        line105[0],
        Statement::DrawImage {
            file: "W_MAN.YBM".to_string(),
            x: Expr::Const(30),
            y: Expr::Const(109),
            mode: Expr::Const(0)
        }
    );

    // Line 9000: GOSUB 4000+A
    let line9000 = &prog.lines[&9000];
    assert_eq!(
        line9000[0],
        Statement::Gosub(Expr::Binary(
            Box::new(Expr::Const(4000)),
            BinOp::Add,
            Box::new(Expr::Var(b'A'))
        ))
    );
}

#[test]
fn test_unrecognized_instruction_captured() {
    let code = "999 FOOBAR BAZ 123\n1000 DRAWIMAGE";
    let prog = ScriptProgram::parse(code);

    let l999 = &prog.lines[&999];
    assert!(matches!(&l999[0], Statement::Unknown { raw, reason } if raw == "FOOBAR BAZ 123"));

    let l1000 = &prog.lines[&1000];
    assert!(matches!(&l1000[0], Statement::Unknown { .. }));
}

#[test]
fn test_parse_disc_tb_and_weight_scripts() {
    let data_dir = Path::new(r"I:\DATA\VCD_DATA");
    if !data_dir.exists() {
        return;
    }

    // Parse T_B.CHM script
    let tb_path = data_dir.join("T_B.CHM");
    if tb_path.exists() {
        let bytes = std::fs::read(&tb_path).unwrap();
        let doc = CompHtmlDoc::parse(&bytes).unwrap();
        let script = doc.get_script().expect("T_B.CHM must have VCDSCRIPT");
        let prog = ScriptProgram::parse(script);
        println!("T_B parsed lines count: {}", prog.lines.len());
        assert!(prog.lines.len() >= 10);
        assert!(prog.lines.contains_key(&20));
        assert!(prog.lines.contains_key(&100));
        assert!(prog.lines.contains_key(&9000));
    }

    // Parse WEIGHT.CHM script
    let weight_path = data_dir.join("WEIGHT.CHM");
    if weight_path.exists() {
        let bytes = std::fs::read(&weight_path).unwrap();
        let doc = CompHtmlDoc::parse(&bytes).unwrap();
        let script = doc.get_script().expect("WEIGHT.CHM must have VCDSCRIPT");
        let prog = ScriptProgram::parse(script);
        println!("WEIGHT parsed lines count: {}", prog.lines.len());
        assert!(prog.lines.len() >= 20);
        assert!(prog.lines.contains_key(&5));
        assert!(prog.lines.contains_key(&200));
        assert!(prog.lines.contains_key(&9000));
    }
}
