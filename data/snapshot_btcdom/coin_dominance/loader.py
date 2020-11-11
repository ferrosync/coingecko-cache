#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import glob
import json
from tqdm import tqdm
import os.path
from multiprocessing import Pool, Queue
import itertools
import datetime

GLOB_PATH = '/home/joshua/repos/btcdom/coingecko-cache/data/snapshot_btcdom/coin_dominance/**/*.json'

def process_file(paths):
    subresult = {}
    
    files = []
    for path in paths:
        f = open(path, 'r')
        files.append(f)
        raw = f.read()
        filename = os.path.basename(path)
        unix_ms = os.path.splitext(filename)[0]
        subresult[unix_ms] = raw
        
    for f in files:
        f.close()
    
    return subresult


result = {}
def get_files(): return glob.iglob(GLOB_PATH, recursive=True)
num_files = sum(1 for _ in get_files())

def igrouping(n, iterable):
    it = iter(iterable)
    while True:
        chunk_it = itertools.islice(it, n)
        try:
            first_el = next(chunk_it)
        except StopIteration:
            return
        yield list(itertools.chain((first_el,), chunk_it))

with Pool(processes=8) as pool:
    q = Queue()
    progress = tqdm(total=num_files)
    k_chunk = 100
    for subresult in pool.imap_unordered(process_file, igrouping(k_chunk, get_files())):
        result.update(subresult)
        progress.update(k_chunk)

####

import psycopg2
import psycopg2.extras
from uuid import uuid4

conn = psycopg2.connect("host=localhost port=54320 dbname=domfin_coingecko user=postgres password=development_only")

def from_unix_ms(ts):
    return datetime.datetime.fromtimestamp(int(ts) / 1000.0)

AGENT = "py_import"

tups = [{'uuid': str(uuid4()), 'timestamp': from_unix_ms(timestamp_ms), 'data': body, 'json': json.loads(body, parse_float=str)}
        for timestamp_ms, body in result.items()]


# (uuid, agent, timestamp_utc, body, headers)
data_origin = [(x['uuid'], AGENT, x['timestamp'], x['data'], None) for x in tups]

# (data_origin_uuid, agemt, timestamp_utc, coin_id, coin_name, market_cap_usd, market_dominance_percentage)
coin_dominance = [(
    x['uuid'],
    AGENT,
    x['timestamp'],
    coin['id'],
    coin['name'],
    coin['market_cap_usd'],
    coin['dominance_percentage']
    
) for x in tups
    for coin in x['json']]

with conn.cursor() as cur:
    data_origin_query = """
    insert into data_origin (uuid, agent, timestamp_utc, data, metadata) values %s
    """
    
    psycopg2.extras.execute_values(cur,
       data_origin_query, data_origin, template=None, page_size=1000)
    
    coin_dominance_query = """
    insert into coin_dominance (
        data_origin_uuid,
        agent,
        timestamp_utc,
        coin_id,
        coin_name,
        market_cap_usd,
        market_dominance_percentage
    ) values %s
    """
    
    psycopg2.extras.execute_values(cur,
       coin_dominance_query, coin_dominance, template=None, page_size=1000)
    
    conn.commit()

conn.rollback()
