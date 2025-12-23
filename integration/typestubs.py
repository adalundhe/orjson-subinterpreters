# SPDX-License-Identifier: (Apache-2.0 OR MIT)
# Copyright ijl (2023), Eric Jolibois (2022)

import hyperjson

hyperjson.JSONDecodeError(msg="the_msg", doc="the_doc", pos=1)

hyperjson.dumps(hyperjson.Fragment(b"{}"))
