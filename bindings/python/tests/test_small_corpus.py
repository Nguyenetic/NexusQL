from nexusql import NexusDB


def test_lightweight_name_ranking():
    with NexusDB(":memory:") as db:
        db.execute("CREATE TABLE names (name TEXT)")
        db.bulk_insert("names", ["name"], [("EECM-210",), ("EECM021",), ("EECM-021",)])
        rows = db.execute("SELECT name, COLBERT_SCORE($1,name) AS score FROM names ORDER BY score DESC", ["EECM-021"]).rows
        assert [r["name"] for r in rows] == ["EECM-021", "EECM021", "EECM-210"]


def test_one_shot_model_blob_and_confidence(tmp_path):
    path = str(tmp_path / "classifier")
    with NexusDB(path) as db:
        model = db.execute("SELECT HDC_LEARN(NULL,$1,$2,$3,$4) AS model", ["ZONE-A", "SW-ALPHA", "Gi1/0/1", 101]).rows[0]["model"]
        assert isinstance(model, bytes)
        db.execute("CREATE TABLE classifier (model BLOB)")
        db.execute("INSERT INTO classifier VALUES ($1)", [model])
        db.execute("UPDATE classifier SET model = HDC_LEARN(model,'ZONE-B','SW-BETA','Gi1/0/1',202)")
    with NexusDB(path) as db:
        row = db.execute("SELECT HDC_PREDICT(model,$1,$2,$3) AS zone, HDC_CONFIDENCE(model,$1,$2,$3) AS confidence FROM classifier", ["SW-ALPHA", "Gi1/0/17", 101]).rows[0]
        assert row["zone"] == "ZONE-A"
        assert 0.1 < row["confidence"] <= 1.0
