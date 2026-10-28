"""Client-side blacklist using e621's syntax.

One rule per line. A post is hidden when a line matches:
  * every plain tag on the line is present
  * no ``-tag`` on the line is present
  * at least one ``~tag`` is present (if the line has any)
Supported metatags: ``rating:s|q|e`` (or safe/questionable/explicit),
``type:<ext>``, ``id:<n>``. Lines starting with ``#`` are comments.
"""
from __future__ import annotations

from dataclasses import dataclass

from .api import Post


@dataclass
class Rule:
    text: str
    required: list[str]
    excluded: list[str]
    any_of: list[str]


def _has(tag: str, tags: set[str], post: Post) -> bool:
    if tag.startswith("rating:"):
        return post.rating == tag[7:8]
    if tag.startswith("type:"):
        return post.ext == tag[5:]
    if tag.startswith("id:"):
        return str(post.id) == tag[3:]
    return tag in tags


class Blacklist:
    def __init__(self, text: str = ""):
        self.rules: list[Rule] = []
        for line in text.splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            toks = line.lower().split()
            req = [t for t in toks if t[0] not in "-~"]
            exc = [t[1:] for t in toks if t[0] == "-" and len(t) > 1]
            any_ = [t[1:] for t in toks if t[0] == "~" and len(t) > 1]
            if not req and not any_:
                continue  # a line of only exclusions would hide everything
            self.rules.append(Rule(line, req, exc, any_))

    def match(self, post: Post) -> Rule | None:
        tags = post.all_tags
        for r in self.rules:
            if (all(_has(t, tags, post) for t in r.required)
                    and not any(_has(t, tags, post) for t in r.excluded)
                    and (not r.any_of or any(_has(t, tags, post) for t in r.any_of))):
                return r
        return None

    def is_blocked(self, post: Post) -> bool:
        return self.match(post) is not None

    def filter(self, posts: list[Post]) -> tuple[list[Post], int]:
        kept = [p for p in posts if not self.is_blocked(p)]
        return kept, len(posts) - len(kept)
