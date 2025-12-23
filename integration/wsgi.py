# SPDX-License-Identifier: (Apache-2.0 OR MIT)
# Copyright ijl (2018-2025)

from datetime import datetime, timezone
from uuid import uuid4

from flask import Flask

import hyperjson

app = Flask(__name__)

NOW = datetime.now(timezone.utc)


@app.route("/")
def root():
    data = {
        "uuid": uuid4(),
        "updated_at": NOW,
        "data": [1, 2.2, None, True, False, hyperjson.Fragment(b"{}")],
    }
    payload = hyperjson.dumps(
        data,
        option=hyperjson.OPT_NAIVE_UTC | hyperjson.OPT_OMIT_MICROSECONDS,
    )
    return app.response_class(
        response=payload,
        status=200,
        mimetype="application/json; charset=utf-8",
    )
