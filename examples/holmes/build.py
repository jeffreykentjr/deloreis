#!/usr/bin/env python3
"""Build and replay the sourced Holmes casebook through the authoring server.

Source decisions and modelling choices are explained in sources.md.
"""
import json
import pathlib
import subprocess
import sys
import tempfile

binary = sys.argv[1] if len(sys.argv) > 1 else "deloreis"
decisions = ["d-0001", "d-0002", "d-0003", "d-0004", "d-0005", "d-0006", "d-0007"]
entities = [
    ("person", "watson", "John Watson", "d-0001"),
    ("person", "holmes", "Sherlock Holmes", "d-0001"),
    ("person", "stamford", "Stamford", "d-0001"),
    ("person", "murray", "Murray", "d-0001"),
    ("person", "kate-whitney", "Kate Whitney", "d-0003"),
    ("person", "isa-whitney", "Isa Whitney", "d-0003"),
    ("person", "elias-whitney", "Elias Whitney", "d-0003"),
    ("person", "moriarty", "Professor Moriarty", "d-0004"),
    ("person", "peter-steiler", "Peter Steiler the elder", "d-0004"),
    ("person", "mary-morstan", "Mary Morstan", "d-0002"),
    ("person", "captain-morstan", "Captain Morstan", "d-0002"),
    ("person", "cecil-forrester", "Mrs Cecil Forrester", "d-0002"),
    ("person", "thaddeus-sholto", "Thaddeus Sholto", "d-0002"),
    ("person", "bartholomew-sholto", "Bartholomew Sholto", "d-0002"),
    ("person", "major-sholto", "Major John Sholto", "d-0002"),
    ("person", "mrs-hudson", "Mrs Hudson", "d-0002"),
    ("person", "mycroft", "Mycroft Holmes", "d-0005"),
    ("person", "ronald-adair", "Ronald Adair", "d-0005"),
    ("person", "sebastian-moran", "Colonel Sebastian Moran", "d-0005"),
    ("person", "augustus-moran", "Sir Augustus Moran", "d-0005"),
    ("person", "earl-of-maynooth", "Earl of Maynooth", "d-0005"),
    ("person", "oscar-meunier", "Oscar Meunier", "d-0005"),
    ("person", "neville-st-clair", "Neville St Clair", "d-0003"),
    ("person", "mrs-st-clair", "Mrs St Clair", "d-0003"),
    ("person", "athelney-jones", "Athelney Jones", "d-0002"),
    ("person", "le-villard", "François Le Villard", "d-0002"),
    ("person", "jonathan-small", "Jonathan Small", "d-0002"),
    ("person", "tonga", "Tonga", "d-0002"),
    ("place", "london", "London", "d-0001"),
    ("place", "maiwand", "Maiwand", "d-0001"),
    ("place", "bombay", "Bombay", "d-0001"),
    ("place", "netley", "Netley", "d-0001"),
    ("place", "strand", "The Strand", "d-0001"),
    ("thing", "orontes", "Orontes", "d-0001"),
    ("place", "peshawar", "Peshawar", "d-0001"),
    ("place", "portsmouth", "Portsmouth", "d-0001"),
    ("place", "criterion-bar", "Criterion Bar", "d-0001"),
    ("place", "holborn", "Holborn", "d-0001"),
    ("place", "meiringen", "Meiringen", "d-0004"),
    ("place", "edinburgh", "Edinburgh", "d-0002"),
    ("place", "langham-hotel", "Langham Hotel", "d-0002"),
    ("place", "upper-norwood", "Upper Norwood", "d-0002"),
    ("place", "pondicherry-lodge", "Pondicherry Lodge", "d-0002"),
    ("place", "andaman-islands", "Andaman Islands", "d-0002"),
    ("place", "reichenbach", "Reichenbach Falls", "d-0004"),
    ("place", "tibet", "Tibet", "d-0005"),
    ("place", "persia", "Persia", "d-0005"),
    ("place", "mecca", "Mecca", "d-0005"),
    ("place", "khartoum", "Khartoum", "d-0005"),
    ("place", "sherpur", "Sherpur", "d-0005"),
    ("place", "cabul", "Cabul", "d-0005"),
    ("place", "florence", "Florence", "d-0005"),
    ("place", "camden-house", "Camden House", "d-0005"),
    ("place", "the-cedars", "The Cedars", "d-0003"),
    ("place", "swandam-lane", "Upper Swandam Lane", "d-0003"),
    ("thing", "wax-bust", "The wax bust", "d-0005"),
    ("place", "baker-street", "Baker Street", "d-0005"),
    ("place", "baker-street-lodgings", "Holmes’s Baker Street rooms", "d-0005"),
    ("era", "years-1880s", "The 1880s", "d-0006"),
    ("era", "year-1891", "1891", "d-0006"),
    ("era", "year-1894", "1894", "d-0006"),
]
facts = [
    ("watson", "first-name", "john", "d-0001"),
    ("watson", "name-used-by-wife", "james", "d-0003"),
    ("watson", "profession", "doctor", "d-0001"),
    ("watson", "studied-at", "university-of-london", "d-0001"),
    ("watson", "trained-at", "netley", "d-0001"),
    ("watson", "landed-in", "bombay", "d-0001"),
    ("watson", "returned-on", "orontes", "d-0001"),
    ("watson", "stayed-on", "strand", "d-0001"),
    ("watson", "held-rank", "assistant-surgeon", "d-0001"),
    ("watson", "served-with", "berkshires", "d-0001"),
    ("watson", "contracted", "enteric-fever", "d-0001"),
    ("watson", "served-at", "maiwand", "d-0001"),
    ("murray", "assisted", "watson", "d-0001"),
    ("watson", "recovering-at", "peshawar", "d-0001"),
    ("watson", "landed-at", "portsmouth", "d-0001"),
    ("watson", "met-stamford-at", "criterion-bar", "d-0001"),
    ("watson", "lunched-at", "holborn", "d-0001"),
    ("watson", "sought-lodgings-in", "london", "d-0001"),
    ("holmes", "sought-lodgings-in", "london", "d-0001"),
    ("holmes", "occupation", "consulting-detective", "d-0002"),
    ("le-villard", "consulted", "holmes", "d-0002"),
    ("le-villard", "profession", "detective", "d-0002"),
    ("mary-morstan", "received", "pearls", "d-0002"),
    ("mary-morstan", "sought-help-from", "holmes", "d-0002"),
    ("captain-morstan", "belonged-to", "bombay-infantry", "d-0002"),
    ("major-sholto", "belonged-to", "bombay-infantry", "d-0002"),
    ("major-sholto", "served-at", "andaman-islands", "d-0002"),
    ("kate-whitney", "married-to", "isa-whitney", "d-0003"),
    ("isa-whitney", "brother-of", "elias-whitney", "d-0003"),
    ("watson", "medical-adviser-of", "isa-whitney", "d-0003"),
    ("mary-morstan", "employed-by", "cecil-forrester", "d-0002"),
    ("mary-morstan", "occupation", "governess", "d-0002"),
    ("mary-morstan", "father", "captain-morstan", "d-0002"),
    ("mary-morstan", "boarded-in", "edinburgh", "d-0002"),
    ("captain-morstan", "stayed-at", "langham-hotel", "d-0002"),
    ("captain-morstan", "served-at", "andaman-islands", "d-0002"),
    ("captain-morstan", "friend", "major-sholto", "d-0002"),
    ("major-sholto", "lived-in", "upper-norwood", "d-0002"),
    ("major-sholto", "home", "pondicherry-lodge", "d-0002"),
    ("thaddeus-sholto", "father", "major-sholto", "d-0002"),
    ("bartholomew-sholto", "father", "major-sholto", "d-0002"),
    ("thaddeus-sholto", "twin-brother", "bartholomew-sholto", "d-0002"),
    ("mrs-hudson", "landlady-of", "holmes", "d-0002"),
    ("holmes", "brother", "mycroft", "d-0005"),
    ("holmes", "visited", "florence", "d-0005"),
    ("holmes", "travelled-in", "tibet", "d-0005"),
    ("mycroft", "preserved-rooms-in", "baker-street", "d-0005"),
    ("camden-house", "opposite", "baker-street-lodgings", "d-0005"),
    # Study in Scarlet, part I, chapter 1.
    ("watson", "medical-degree-year", "year-1878", "d-0001"),
    ("watson", "original-regiment", "fifth-northumberland-fusiliers", "d-0001"),
    ("watson", "battle", "maiwand", "d-0001"),
    ("watson", "hospitalized-at", "peshawar", "d-0001"),
    ("watson", "homeward-arrival", "portsmouth", "d-0001"),
    ("watson", "daily-income", "eleven-shillings-sixpence", "d-0001"),
    ("stamford", "worked-at", "barts", "d-0001"),
    ("stamford", "introduced", "holmes", "d-0001"),
    ("holmes", "studied", "chemistry", "d-0001"),
    ("holmes", "worked-in", "hospital-laboratory", "d-0001"),
    ("murray", "role", "watson-orderly", "d-0001"),
    ("orontes", "type", "troopship", "d-0001"),
    # The Sign of the Four, chapters 1–4.
    ("mary-morstan", "mother", "deceased", "d-0002"),
    ("mary-morstan", "school-city", "edinburgh", "d-0002"),
    ("mary-morstan", "school-until-age", "seventeen", "d-0002"),
    ("mary-morstan", "pearl-count", "six", "d-0002"),
    ("mary-morstan", "advertisement-newspaper", "times", "d-0002"),
    ("mary-morstan", "advertisement-date", "may-4-1882", "d-0002"),
    ("captain-morstan", "leave-began", "year-1878", "d-0002"),
    ("captain-morstan", "disappeared", "december-3-1878", "d-0002"),
    ("captain-morstan", "hotel-luggage", "left-behind", "d-0002"),
    ("captain-morstan", "military-rank", "captain", "d-0002"),
    ("major-sholto", "military-rank", "major", "d-0002"),
    ("major-sholto", "died", "april-28-1882", "d-0002"),
    ("thaddeus-sholto", "sibling", "bartholomew-sholto", "d-0002"),
    ("thaddeus-sholto", "patron", "arts", "d-0002"),
    ("thaddeus-sholto", "asked-advice-of", "watson", "d-0002"),
    ("bartholomew-sholto", "sibling", "thaddeus-sholto", "d-0002"),
    ("le-villard", "translated-work-of", "holmes", "d-0002"),
    ("le-villard", "worked-in", "france", "d-0002"),
    ("holmes", "wrote-about", "tobacco-ash", "d-0002"),
    ("holmes", "wrote-about", "footsteps", "d-0002"),
    ("holmes", "wrote-about", "trade-and-hands", "d-0002"),
    ("cecil-forrester", "employed", "mary-morstan", "d-0002"),
    # The Man with the Twisted Lip.
    ("isa-whitney", "wife", "kate-whitney", "d-0003"),
    ("isa-whitney", "addiction", "opium", "d-0003"),
    ("elias-whitney", "brother", "isa-whitney", "d-0003"),
    ("elias-whitney", "former-occupation", "college-principal", "d-0003"),
    ("kate-whitney", "sought-help-from", "watson", "d-0003"),
    ("kate-whitney", "school-friend-of", "watson-wife", "d-0003"),
    ("watson", "sought", "isa-whitney", "d-0003"),
    # The Final Problem.
    ("peter-steiler", "managed", "englischer-hof", "d-0004"),
    ("peter-steiler", "spoke", "english", "d-0004"),
    ("peter-steiler", "previous-employer", "grosvenor-hotel", "d-0004"),
    ("holmes", "stayed-in", "meiringen", "d-0004"),
    ("watson", "stayed-in", "meiringen", "d-0004"),
    ("holmes", "encountered", "moriarty", "d-0004"),
    ("watson", "found", "holmes-alpine-stock", "d-0004"),
    ("watson", "received-note-from", "holmes", "d-0004"),
    # The Empty House, Holmes's account.
    ("holmes", "kept-informed", "mycroft", "d-0005"),
    ("holmes", "travel-alias", "sigerson", "d-0005"),
    ("holmes", "visited-after-florence", "tibet", "d-0005"),
    ("holmes", "visited", "persia", "d-0005"),
    ("holmes", "visited", "mecca", "d-0005"),
    ("holmes", "visited", "khartoum", "d-0005"),
    ("holmes", "researched-in", "montpellier", "d-0005"),
    ("ronald-adair", "died-in", "london", "d-0005"),
    ("sebastian-moran", "former-regiment", "first-bangalore-pioneers", "d-0005"),
    ("sebastian-moran", "born-in", "london", "d-0005"),
    ("sebastian-moran", "birth-year", "year-1840", "d-0005"),
    ("sebastian-moran", "associate-of", "moriarty", "d-0005"),
    # The Empty House: Adair's case and Holmes's return.
    ("ronald-adair", "father", "earl-of-maynooth", "d-0005"),
    ("ronald-adair", "sister", "hilda-adair", "d-0005"),
    ("ronald-adair", "address", "427-park-lane", "d-0005"),
    ("ronald-adair", "former-fiancee", "edith-woodley", "d-0005"),
    ("ronald-adair", "death-date", "march-30-1894", "d-0005"),
    ("ronald-adair", "played", "whist", "d-0005"),
    ("ronald-adair", "card-club", "bagatelle", "d-0005"),
    ("ronald-adair", "card-club", "baldwin", "d-0005"),
    ("ronald-adair", "card-club", "cavendish", "d-0005"),
    ("ronald-adair", "played-cards-with", "sebastian-moran", "d-0005"),
    ("holmes", "observed-from", "camden-house", "d-0005"),
    ("holmes", "used-as-decoy", "wax-bust", "d-0005"),
    ("holmes", "sentry-identified-as", "parker", "d-0005"),
    ("mrs-hudson", "moved", "wax-bust", "d-0005"),
    ("mrs-hudson", "cared-for", "baker-street-lodgings", "d-0005"),
    ("mycroft", "provided-funds-to", "holmes", "d-0005"),
    ("holmes", "visited-lhasa", "lhasa", "d-0005"),
    ("holmes", "tibet-duration", "two-years", "d-0005"),
    ("holmes", "montpellier-study", "coal-tar-derivatives", "d-0005"),
    ("holmes", "learned-of", "park-lane-mystery", "d-0005"),
    ("holmes", "returned-to", "london", "d-0005"),
    ("holmes", "disguised-as", "book-merchant", "d-0005"),
    ("holmes", "disclosed-himself-to", "watson", "d-0005"),
    ("holmes", "recognized-enemy", "sebastian-moran", "d-0005"),
    ("sebastian-moran", "military-rank", "colonel", "d-0005"),
    ("sebastian-moran", "father", "augustus-moran", "d-0005"),
    ("sebastian-moran", "educated-at", "eton", "d-0005"),
    ("sebastian-moran", "educated-at", "oxford", "d-0005"),
    ("sebastian-moran", "address", "conduit-street", "d-0005"),
    ("sebastian-moran", "club", "bagatelle", "d-0005"),
    ("sebastian-moran", "club", "tankerville", "d-0005"),
    ("sebastian-moran", "club", "anglo-indian", "d-0005"),
    ("sebastian-moran", "served-in", "afghan-campaign", "d-0005"),
    ("sebastian-moran", "served-in", "jowaki-campaign", "d-0005"),
    ("sebastian-moran", "wrote", "heavy-game-western-himalayas", "d-0005"),
    ("sebastian-moran", "wrote", "three-months-in-jungle", "d-0005"),
    ("moriarty", "commissioned", "air-gun", "d-0005"),
    ("camden-house", "used-by", "holmes", "d-0005"),
    ("camden-house", "was", "empty", "d-0005"),
    # Additional details from Mary's statement.
    ("mary-morstan", "father-returned", "year-1878", "d-0002"),
    ("mary-morstan", "father-hotel", "langham-hotel", "d-0002"),
    ("mary-morstan", "reported-disappearance-to", "police", "d-0002"),
    ("mary-morstan", "employer-referred-her-to", "holmes", "d-0002"),
    ("mary-morstan", "showed-pearls-to", "holmes", "d-0002"),
    ("captain-morstan", "knew", "major-sholto", "d-0002"),
    ("captain-morstan", "overseas-service", "andaman-islands", "d-0002"),
    ("major-sholto", "retired-to", "upper-norwood", "d-0002"),
    ("major-sholto", "child", "thaddeus-sholto", "d-0002"),
    ("major-sholto", "child", "bartholomew-sholto", "d-0002"),
    ("thaddeus-sholto", "invited", "mary-morstan", "d-0002"),
    ("thaddeus-sholto", "accompanied-to", "pondicherry-lodge", "d-0002"),
    # The Empty House: the trap and Moran's background.
    ("wax-bust", "depicts", "holmes", "d-0005"),
    ("wax-bust", "made-by", "oscar-meunier", "d-0005"),
    ("wax-bust", "material", "wax", "d-0005"),
    ("wax-bust", "moulding-duration", "several-days", "d-0005"),
    ("wax-bust", "placed-in", "baker-street-lodgings", "d-0005"),
    ("wax-bust", "wore", "holmes-dressing-gown", "d-0005"),
    ("wax-bust", "struck-by", "bullet", "d-0005"),
    ("oscar-meunier", "based-in", "grenoble", "d-0005"),
    ("mrs-hudson", "recovered", "bullet", "d-0005"),
    ("mrs-hudson", "reported-to", "holmes", "d-0005"),
    ("camden-house", "room-used", "front-room", "d-0005"),
    ("holmes", "planned-arrest-with", "lestrade", "d-0005"),
    ("holmes", "examined", "air-gun", "d-0005"),
    ("holmes", "identified-wax-maker", "oscar-meunier", "d-0005"),
    ("sebastian-moran", "campaign-charasiab", "mentioned-in-despatches", "d-0005"),
    ("sebastian-moran", "served-at", "sherpur", "d-0005"),
    ("sebastian-moran", "served-at", "cabul", "d-0005"),
    ("sebastian-moran", "first-book-year", "year-1881", "d-0005"),
    ("sebastian-moran", "second-book-year", "year-1884", "d-0005"),
    ("moriarty", "used-agent", "sebastian-moran", "d-0005"),
    ("ronald-adair", "mother-treatment", "cataract-operation", "d-0005"),
    ("ronald-adair", "engagement-ended", "mutual-consent", "d-0005"),
    ("ronald-adair", "died-between", "ten-and-eleven-twenty", "d-0005"),
    ("ronald-adair", "position-in-family", "second-son", "d-0005"),
    # The Man with the Twisted Lip: Mrs St Clair's evidence.
    ("mrs-st-clair", "husband", "neville-st-clair", "d-0003"),
    ("mrs-st-clair", "home", "the-cedars", "d-0003"),
    ("mrs-st-clair", "saw-husband-in", "swandam-lane", "d-0003"),
    ("mrs-st-clair", "received-letter-from", "neville-st-clair", "d-0003"),
    ("mrs-st-clair", "received", "signet-ring", "d-0003"),
    ("mrs-st-clair", "received-letter-postmark", "gravesend", "d-0003"),
    ("mrs-st-clair", "consulted", "holmes", "d-0003"),
    ("mrs-st-clair", "hosted", "holmes", "d-0003"),
    ("mrs-st-clair", "hosted", "watson", "d-0003"),
    ("neville-st-clair", "wife", "mrs-st-clair", "d-0003"),
    ("neville-st-clair", "home", "the-cedars", "d-0003"),
    ("neville-st-clair", "sent-letter-to", "mrs-st-clair", "d-0003"),
    ("neville-st-clair", "sent", "signet-ring", "d-0003"),
    ("the-cedars", "hosted", "holmes", "d-0003"),
    ("the-cedars", "hosted", "watson", "d-0003"),
    ("swandam-lane", "contains", "bar-of-gold", "d-0003"),



]

def cited(decision):
    return {"kind": "cited", "decisions": [decision], "state": "ratified"}

with tempfile.TemporaryDirectory(prefix="dli-holmes-") as temp:
    project = pathlib.Path(temp) / "world"
    subprocess.run([binary, "init", str(project)], check=True, stdout=subprocess.DEVNULL)
    server = subprocess.Popen([binary, "mcp", "--tools", "all", "--project", str(project)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    sequence = 0
    history = []
    def call(name, arguments, allow_findings=False):
        global sequence
        sequence += 1
        history.append({"tool": name, "arguments": arguments})
        server.stdin.write(json.dumps({"jsonrpc": "2.0", "id": sequence, "method": "tools/call", "params": {"name": name, "arguments": arguments}}) + "\n")
        server.stdin.flush()
        frame = json.loads(server.stdout.readline())
        result = json.loads(frame["result"]["content"][0]["text"])
        if not result["ok"]:
            raise RuntimeError(f"{name}: {result}")
        for finding in result.get("result", {}).get("findings", []):
            if finding.get("severity") == "error" and not allow_findings:
                raise RuntimeError(f"{name}: {finding}")
        return result["result"]
    try:
        workspace = call("author.open", {"scope": "world"})["workspace"]
        for decision in decisions:
            call("author.record_decision", {"workspace": workspace, "decision": decision})
        for kind, identifier, name, decision in entities:
            call("author.add_entity", {"workspace": workspace, "kind": kind, "id": identifier, "name": name, "provenance": cited(decision)})
        timeline_start = len(history)
        events = [
            ("scandal", "A Scandal in Bohemia", "years-1880s", "d-0007"),
            ("mary-case", "Mary Morstan consults Holmes", "years-1880s", "d-0002"),
            ("reichenbach-encounter", "The Reichenbach encounter", "year-1891", "d-0004"),
            ("return", "Holmes returns", "year-1894", "d-0005"),
        ]
        for identifier, name, era, decision in events:
            call("author.add_event", {"workspace": workspace, "id": identifier, "name": name, "era": era, "provenance": cited(decision)})
        for earlier, later in [("years-1880s", "year-1891"), ("year-1891", "year-1894")]:
            call("author.add_before", {"workspace": workspace, "earlier": earlier, "later": later, "provenance": cited("d-0006")})
        timeline_calls = history[timeline_start:]
        call("author.declare_functional", {"workspace": workspace, "pred": "first-name", "provenance": cited("d-0006")})
        for entity, key, value, decision in facts:
            call("author.add_fact", {"workspace": workspace, "entity": entity, "key": key, "value": value, "provenance": cited(decision)})
        wound_start = len(history)
        for value, source in [("shoulder", "study-in-scarlet"), ("leg", "sign-of-four")]:
            call("author.add_fact", {"workspace": workspace, "entity": "watson", "key": "reported-wound-location", "value": value, "provenance": {"kind": "proposed", "origin": source}})
        result = call("author.materialize", {"workspace": workspace, "rendered": True})
        # Serialize the recorded decisions and server-rendered declarations for
        # the CLI timeline. The editable world itself was authored via MCP.
        lines = result["rendered"]
        text = lines[:2] + ["ratify " + decision for decision in decisions] + [""] + lines[2:]
        corpus = pathlib.Path(temp) / "opening.dli"
        corpus.write_text("\n".join(text) + "\n", encoding="utf-8")
        timeline = subprocess.run([binary, "timeline", "show", "--corpus", str(corpus), "--markdown"], text=True, capture_output=True, check=True).stdout
        checked_corpus = subprocess.run([binary, "check", str(corpus)], text=True, capture_output=True)
        if checked_corpus.returncode != 0 or checked_corpus.stdout != "checked 1 document(s): clean\n":
            raise RuntimeError(f"corpus check failed: {checked_corpus.stdout} {checked_corpus.stderr}")
        if "`mary-case` → `scandal`" in timeline or "`scandal` → `mary-case`" in timeline:
            raise RuntimeError("the dates of Mary’s case and the scandal were incorrectly ordered")
        def query(predicate, value):
            atom = {"pred": predicate, "args": [{"Sym": "watson"}, {"Sym": value}]}
            return call("author.query", {"workspace": workspace, "goal_atom": atom, "layer": "all"})["rows"]
        wound = {value: query("reported-wound-location", value) for value in ("shoulder", "leg")}
        wound_why = {}
        for value in ("shoulder", "leg"):
            atom = {"pred": "reported-wound-location", "args": [{"Sym": "watson"}, {"Sym": value}]}
            wound_why[value] = call("author.why", {"workspace": workspace, "goal_atom": atom})
        if any(rows[0]["status"] != "possible" for rows in wound.values()):
            raise RuntimeError(f"unexpected wound results: {wound}")
        wound_calls = history[wound_start:]
        # Stage both interpretations rather than committing a contradictory world.
        chronology_start = len(history)
        call("author.add_before", {"workspace": workspace, "earlier": "mary-case", "later": "scandal", "provenance": cited("d-0002")}, allow_findings=True)
        call("author.add_before", {"workspace": workspace, "earlier": "scandal", "later": "mary-case", "provenance": {"kind": "cited", "decisions": ["d-0002", "d-0006", "d-0007"], "state": "ratified"}}, allow_findings=True)
        chronology_check = call("author.check", {"workspace": workspace}, allow_findings=True)
        chronology_findings = [f for f in chronology_check["findings"] if f["code"] == "continuity.chronology-cycle"]
        if len(chronology_findings) != 1 or not {"d-0002", "d-0007"}.issubset(set(chronology_findings[0]["citations"])):
            raise RuntimeError(f"unexpected chronology conflict: {chronology_check}")
        timeline_calls.extend(history[chronology_start:])
        call("author.discard", {"workspace": workspace})
        workspace = call("author.open", {"scope": "world"})["workspace"]
        name_start = len(history)
        staged_name = call("author.add_fact", {"workspace": workspace, "entity": "watson", "key": "first-name", "value": "james", "provenance": cited("d-0003")}, allow_findings=True)
        checked = call("author.check", {"workspace": workspace}, allow_findings=True)
        conflicts = [finding for finding in checked["findings"] if finding["code"] == "continuity.fd-violation"]
        if len(conflicts) != 1 or set(conflicts[0]["citations"]) != {"d-0001", "d-0003"}:
            raise RuntimeError(f"unexpected conflict: {checked}")
        name_calls = history[name_start:]
        call("author.discard", {"workspace": workspace})
        workspace = call("author.open", {"scope": "world"})["workspace"]
        def atom(pred, subject, value):
            return {"pred": pred, "args": [{"Sym": subject}, {"Sym": value}]}
        rule_start = len(history)
        variables = [{"Var": "person"}, {"Var": "name"}]
        rule = call("author.add_rule", {"workspace": workspace, "name": "wife-name-is-first-name", "head": {"pred": "first-name", "args": variables}, "body": [{"atom": {"pred": "name-used-by-wife", "args": variables}, "negated": False}], "provenance": cited("d-0006")}, allow_findings=True)
        rule_checked = call("author.check", {"workspace": workspace}, allow_findings=True)
        rule_conflicts = [finding for finding in rule_checked["findings"] if finding["code"] == "continuity.derived-fd-violation"]
        if len(rule_conflicts) != 1 or not {"d-0001", "d-0003", "d-0006"}.issubset(set(rule_conflicts[0]["citations"])):
            raise RuntimeError(f"unexpected rule-derived conflict: {rule_checked}")
        rule_calls = history[rule_start:]
        call("author.discard", {"workspace": workspace})
        workspace = call("author.open", {"scope": "world"})["workspace"]
        retcon_start = len(history)
        call("author.record_decision", {"workspace": workspace, "decision": "d-0008"})
        death_added = call("author.add_fact", {"workspace": workspace, "entity": "holmes", "key": "reichenbach-outcome", "value": "died", "provenance": {"kind": "cited", "decisions": ["d-0004", "d-0008"], "state": "ratified"}})
        call("author.materialize", {"workspace": workspace})
        def outcome(ws, value):
            atom = {"pred": "reichenbach-outcome", "args": [{"Sym": "holmes"}, {"Sym": value}]}
            return call("author.query", {"workspace": ws, "goal_atom": atom, "layer": "all"})["rows"]
        before = outcome(workspace, "died")
        if before[0]["status"] != "certain":
            raise RuntimeError(f"unexpected earlier ruling: {before}")
        call("author.branch", {"workspace": workspace, "action": "create", "name": "holmes-stayed-dead", "from": "main"})
        call("author.defeat", {"workspace": workspace, "claim_id": before[0]["claim_ids"][0], "decision": "d-0009", "defeated_decision": "d-0008"})
        survived_added = call("author.add_fact", {"workspace": workspace, "entity": "holmes", "key": "reichenbach-outcome", "value": "survived", "provenance": {"kind": "cited", "decisions": ["d-0005", "d-0009"], "state": "ratified"}})
        call("author.materialize", {"workspace": workspace})
        after = outcome(workspace, "survived")
        old_after = outcome(workspace, "died")
        why_old = call("author.why", {"workspace": workspace, "goal_atom": {"pred": "reichenbach-outcome", "args": [{"Sym": "holmes"}, {"Sym": "died"}]}})
        alternate = call("author.open", {"scope": "world", "branch": "holmes-stayed-dead"})["workspace"]
        branch_death = outcome(alternate, "died")
        if why_old["why_not"]["status"] != "dead" or why_old["blocking_defeats"] != [{"defeated_decision": "d-0008", "defeating_decision": "d-0009"}]:
            raise RuntimeError(f"unexpected retcon lineage: {why_old}")
        if after[0]["status"] != "certain" or branch_death[0]["status"] != "certain":
            raise RuntimeError(f"unexpected retcon or branch: {after} {branch_death}")
        retcon_calls = history[retcon_start:]
        branch_calls = [entry for entry in retcon_calls if entry["tool"] == "author.branch" or entry["arguments"].get("branch") == "holmes-stayed-dead" or entry["arguments"].get("workspace") == alternate]
        wiki = call("dli.export.run", {"document": "world", "profile": "static-wiki", "profile_name": "wiki/all-standing"})
        wiki_pages = wiki["emission"]["artifact"]["pages"]
        wound_page = next(page for page in wiki_pages if page["title"] == "reported-wound-location")
        if "h.study-in-scarlet" not in wound_page["content"] or "h.sign-of-four" not in wound_page["content"]:
            raise RuntimeError(f"wiki omitted wound support labels: {wound_page}")
        outcome_page = next(page for page in wiki_pages if page["title"] == "reichenbach-outcome")
        if "`reichenbach-outcome(holmes, died)` — **overturned**" not in outcome_page["content"] or "`reichenbach-outcome(holmes, survived)` — **canon**" not in outcome_page["content"]:
            raise RuntimeError(f"wiki misstated a retcon account: {outcome_page}")
        if not all(decision in outcome_page["content"] for decision in ("d-0004", "d-0005", "d-0008", "d-0009")):
            raise RuntimeError(f"wiki omitted retcon sources: {outcome_page}")
        print(json.dumps({"decisions": decisions, "rendered": result["rendered"], "timeline": timeline, "chronology_conflict": chronology_findings[0], "wound_accounts": wound, "wound_why": wound_why, "name_staged": staged_name["rendered"], "name_conflict": conflicts[0], "rule_rendered": rule["rendered"], "rule_conflict": rule_conflicts[0], "calls": {"name": name_calls, "wounds": wound_calls, "timeline": timeline_calls, "rule": rule_calls, "return": retcon_calls, "branch": branch_calls}, "wiki": wiki, "retcon": {"before": before, "after": after, "old_after": old_after, "why_old": why_old, "branch": branch_death, "death_rendered": death_added["rendered"], "survived_rendered": survived_added["rendered"]}}, indent=2))
    finally:
        server.terminate()
        server.wait()
