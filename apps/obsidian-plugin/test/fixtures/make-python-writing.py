#!/usr/bin/env python3
"""Writes python-writing.json: the material page (`material.render`, `ticked_in`) and the zones page
(`zones.page`) of the research repo (commit 9061581) for invented input in the shape `pkm VAULT material` and
`pkm VAULT zones` print, so the plugin's layout can be compared with it.

  python3 make-python-writing.py RESEARCH_REPO
"""
import datetime, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
NOW = datetime.datetime(2026, 11, 4, 9, 5, 7)

MATERIAL = {
    'draft': 'Bài viết/Vườn rau ban công.md',
    'queries': [['tên bài', 'Vườn rau ban công'], ['Ý một câu', 'Trồng rau trên ban công cần nắng, đất tơi và tưới đều đặn mỗi sáng'],
                ['Dàn ý', 'x' * 130]],
    'methods': {'ohs': 3, 'ohs-hits': 1},
    'items': [
        {'id': 'Vườn/Cây cà chua.md', 'type': 'note', 'found_by': 'ohs', 'rank': 0.12, 'used': False,
         'sections': ['tên bài', 'Dàn ý'], 'ohs_score': 1.0, 'highlights': 2,
         'first_highlights': ['Cà chua cần nắng sáu giờ mỗi ngày', 'Tưới gốc, không tưới lá'], 'wise': 'làm được', 'gist': 'nắng là chính'},
        {'id': 'Trích/Người làm vườn.md', 'type': 'quote', 'found_by': 'ohs', 'rank': 0.1, 'used': False, 'sections': ['Ý một câu'],
         'ohs_score': 0.8, 'highlights': 0, 'first_highlights': []},
        {'id': 'Lưu/Đất trồng.md', 'type': None, 'found_by': 'ohs', 'rank': 0.09, 'used': False, 'sections': ['Dàn ý'],
         'ohs_score': 0.7, 'highlights': 1, 'first_highlights': ['Đất tơi xốp'], 'use': 'tham khảo'},
        {'id': 'Bản đồ/Làm vườn.md', 'type': 'moc', 'found_by': 'ohs', 'rank': 0.05, 'used': False, 'sections': ['tên bài'],
         'ohs_score': 0.5, 'highlights': 0, 'first_highlights': []},
        {'id': 'Lạ/Sổ tay.md', 'type': 'logbook', 'found_by': 'ohs', 'rank': 0.04, 'used': False, 'sections': ['tên bài'],
         'ohs_score': 0.4, 'highlights': 0, 'first_highlights': [], 'why': 'ghi chép đo đạc'},
        {'id': 'Vườn/Phân bón.md', 'type': 'note', 'found_by': 'graph', 'rank': 0.03, 'used': False, 'layer': 'proposed',
         'via': [{'from': 'Vườn/Cây cà chua.md', 'rel': 'related', 'to': 'Vườn/Ủ phân.md', 'status': 'accepted', 'claim': None,
                  'proposed_by': None, 'score': None, 'origin': None},
                 {'from': 'Vườn/Ủ phân.md', 'rel': 'related', 'to': 'Vườn/Phân bón.md', 'status': 'proposed', 'claim': None,
                  'proposed_by': 'model', 'score': 0.8123, 'origin': 'proposal'}],
         'highlights': 0, 'first_highlights': []},
        {'id': 'Vườn/Hạt giống.md', 'type': 'clip', 'found_by': 'graph', 'rank': 0.02, 'used': False, 'layer': 'proposed',
         'via': [{'from': 'Vườn/Cây cà chua.md', 'rel': 'link', 'to': 'Vườn/Hạt giống.md', 'status': 'proposed', 'claim': None,
                  'proposed_by': None, 'score': None, 'origin': None}],
         'highlights': 3, 'first_highlights': ['Hạt ngâm ấm qua đêm']},
        {'id': 'Vườn/Chậu.md', 'type': 'note', 'found_by': 'used', 'used': True, 'highlights': 0, 'first_highlights': []},
    ],
    'missing': [],
    'edges': {'accepted': 12, 'proposed': 3},
    'draft_seed': True,
    'topics': [{'id': 'Vườn/Vườn (chủ đề).md', 'hits': 3}],
}

EMPTY = {'draft': 'Bài viết/Bài mới.md', 'queries': [], 'methods': {}, 'items': [], 'missing': [], 'edges': {},
         'draft_seed': False, 'topics': []}

ZONES = [
    {'label': 'cà chua · ban công', 'score': 21.4, 'notes': 14, 'core': ['Vườn/Cây cà chua.md', 'Vườn/Ban công.md'],
     'with_highlights': 3, 'highlights': 7, 'highlight_weight': 15, 'colours': {'làm được': 4, 'chưa màu': 3}, 'own_ideas': 2,
     'folders': 3, 'top_folders': ['Vườn'], 'posts': ['Bài viết/Vườn rau.md'], 'years': ['2020', '2024'],
     'picks': [{'text': 'Cà chua cần nắng', 'colour': 'làm được'}, {'text': 'y' * 230, 'colour': 'chưa màu'}]},
    {'label': 'đất · phân', 'score': 9.5, 'notes': 6, 'core': ['Vườn/Đất trồng.md'], 'with_highlights': 0, 'highlights': 0,
     'highlight_weight': 0, 'colours': {}, 'own_ideas': 0, 'folders': 1, 'top_folders': ['Vườn'], 'posts': [], 'years': [],
     'picks': []},
]


def main(research):
    sys.path.insert(0, os.path.join(research, 'scripts'))
    sys.path.insert(0, os.path.join(research, 'scripts', 'e11'))
    import material as M
    import zones as Z
    ticked = M.ticked_in('- [x] [[Vườn/Cây cà chua|Cây cà chua]] · note\n- [ ] [[Lưu/Đất trồng|Đất trồng]]\n'
                         '    - [X] [[Vườn/Hạt giống]]\n')
    out = {
        'source': 'scripts/e11/material.py, scripts/e11/zones.py at 9061581',
        'now': NOW.isoformat(),
        'material': MATERIAL,
        'ticked': ticked,
        'page': M.render(MATERIAL, frozenset(ticked), NOW),
        'emptyPage': M.render(EMPTY, frozenset(), NOW),
        'zones': ZONES,
        'zonesPage': Z.page(ZONES, 'Vault', '2026-11-04', 20),
    }
    with open(os.path.join(HERE, 'python-writing.json'), 'w', encoding='utf-8') as fh:
        json.dump(out, fh, ensure_ascii=False, indent=1)
        fh.write('\n')


if __name__ == '__main__':
    main(sys.argv[1])
