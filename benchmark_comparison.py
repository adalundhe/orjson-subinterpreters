#!/usr/bin/env python3
"""Benchmark comparison between original orjson and our subinterpreter-compatible version."""

import time
import sys
import importlib
from datetime import datetime, timezone
from dataclasses import dataclass
from typing import List, Dict, Any, Optional
from uuid import uuid4


@dataclass
class Address:
    street: str
    city: str
    state: str
    zip_code: str
    country: str


@dataclass
class Person:
    id: str
    name: str
    age: int
    email: str
    active: bool
    address: Address
    tags: List[str]
    metadata: Dict[str, Any]
    created_at: datetime
    updated_at: Optional[datetime] = None


def create_complex_structure() -> Dict[str, Any]:
    """Create a complex, realistic JSON-serializable structure."""
    persons = []
    for i in range(50):
        person = Person(
            id=str(uuid4()),
            name=f"Person {i}",
            age=20 + (i % 60),
            email=f"person{i}@example.com",
            active=i % 3 != 0,
            address=Address(
                street=f"{100 + i} Main St",
                city=["New York", "Los Angeles", "Chicago", "Houston", "Phoenix"][i % 5],
                state=["NY", "CA", "IL", "TX", "AZ"][i % 5],
                zip_code=f"{10000 + i:05d}",
                country="USA"
            ),
            tags=[f"tag{j}" for j in range(i % 5)],
            metadata={
                "score": 85.5 + (i % 20),
                "level": i % 10,
                "preferences": {
                    "theme": "dark" if i % 2 == 0 else "light",
                    "notifications": i % 3 == 0,
                    "language": ["en", "es", "fr"][i % 3]
                },
                "history": [j for j in range(i % 10)]
            },
            created_at=datetime.now(timezone.utc),
            updated_at=datetime.now(timezone.utc) if i % 2 == 0 else None
        )
        persons.append(person)
    
    structure = {
        "version": "1.0",
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "metadata": {
            "total_count": len(persons),
            "active_count": sum(1 for p in persons if p.active),
            "tags": list(set(tag for p in persons for tag in p.tags)),
            "cities": list(set(p.address.city for p in persons)),
            "nested": {
                "level1": {
                    "level2": {
                        "level3": {
                            "data": [i for i in range(100)],
                            "strings": [f"string_{i}" for i in range(50)],
                            "mixed": [
                                {"id": i, "value": i * 1.5, "active": i % 2 == 0}
                                for i in range(30)
                            ]
                        }
                    }
                }
            }
        },
        "persons": [
            {
                "id": p.id,
                "name": p.name,
                "age": p.age,
                "email": p.email,
                "active": p.active,
                "address": {
                    "street": p.address.street,
                    "city": p.address.city,
                    "state": p.address.state,
                    "zip_code": p.address.zip_code,
                    "country": p.address.country
                },
                "tags": p.tags,
                "metadata": p.metadata,
                "created_at": p.created_at.isoformat(),
                "updated_at": p.updated_at.isoformat() if p.updated_at else None
            }
            for p in persons
        ],
        "statistics": {
            "age_distribution": {
                "20-30": sum(1 for p in persons if 20 <= p.age < 30),
                "30-40": sum(1 for p in persons if 30 <= p.age < 40),
                "40-50": sum(1 for p in persons if 40 <= p.age < 50),
                "50+": sum(1 for p in persons if p.age >= 50)
            },
            "by_state": {
                state: sum(1 for p in persons if p.address.state == state)
                for state in set(p.address.state for p in persons)
            }
        }
    }
    
    return structure


def benchmark_orjson(orjson_module, name: str, data: Dict[str, Any], iterations: int = 10000):
    """Benchmark serialization and deserialization."""
    print(f"\n{'='*60}")
    print(f"Benchmarking: {name}")
    print(f"{'='*60}")
    
    # Warm-up
    for _ in range(100):
        orjson_module.dumps(data)
        orjson_module.loads(orjson_module.dumps(data))
    
    # Serialization benchmark
    serialized = None
    start = time.perf_counter()
    for _ in range(iterations):
        serialized = orjson_module.dumps(data)
    serialize_time = time.perf_counter() - start
    
    # Deserialization benchmark
    start = time.perf_counter()
    for _ in range(iterations):
        orjson_module.loads(serialized)
    deserialize_time = time.perf_counter() - start
    
    # Round-trip benchmark
    start = time.perf_counter()
    for _ in range(iterations):
        result = orjson_module.loads(orjson_module.dumps(data))
    roundtrip_time = time.perf_counter() - start
    
    serialize_ops_per_sec = iterations / serialize_time
    deserialize_ops_per_sec = iterations / deserialize_time
    roundtrip_ops_per_sec = iterations / roundtrip_time
    
    print(f"Serialization:")
    print(f"  Time: {serialize_time:.4f}s")
    print(f"  Operations/sec: {serialize_ops_per_sec:,.0f}")
    print(f"  Avg time per op: {(serialize_time/iterations)*1e6:.2f}μs")
    
    print(f"\nDeserialization:")
    print(f"  Time: {deserialize_time:.4f}s")
    print(f"  Operations/sec: {deserialize_ops_per_sec:,.0f}")
    print(f"  Avg time per op: {(deserialize_time/iterations)*1e6:.2f}μs")
    
    print(f"\nRound-trip:")
    print(f"  Time: {roundtrip_time:.4f}s")
    print(f"  Operations/sec: {roundtrip_ops_per_sec:,.0f}")
    print(f"  Avg time per op: {(roundtrip_time/iterations)*1e6:.2f}μs")
    
    print(f"\nSerialized size: {len(serialized):,} bytes")
    
    return {
        'name': name,
        'serialize_time': serialize_time,
        'deserialize_time': deserialize_time,
        'roundtrip_time': roundtrip_time,
        'serialize_ops_per_sec': serialize_ops_per_sec,
        'deserialize_ops_per_sec': deserialize_ops_per_sec,
        'roundtrip_ops_per_sec': roundtrip_ops_per_sec,
        'serialized_size': len(serialized)
    }


def main():
    print("Creating complex test structure...")
    test_data = create_complex_structure()
    print(f"Test structure created:")
    print(f"  - {len(test_data['persons'])} persons")
    print(f"  - Nested dictionaries up to 4 levels deep")
    print(f"  - Mixed data types (strings, numbers, booleans, lists, dicts)")
    print(f"  - Datetime strings")
    print(f"  - UUIDs")
    
    results = []
    iterations = 10000
    
    # Benchmark original orjson
    print("\n" + "="*60)
    print("Testing ORIGINAL orjson (PyPI 3.11.4)")
    print("="*60)
    try:
        # Clear any cached imports
        if 'orjson' in sys.modules:
            del sys.modules['orjson']
        import orjson as orjson_original
        result = benchmark_orjson(orjson_original, "Original orjson (PyPI 3.11.4)", test_data, iterations)
        results.append(result)
    except Exception as e:
        print(f"Error loading original orjson: {e}")
        print("Skipping original orjson benchmark")
    
    # Uninstall original and install our version
    print("\n" + "="*60)
    print("Switching to MODIFIED orjson (subinterpreter-compatible)")
    print("="*60)
    import subprocess
    import glob
    subprocess.run([sys.executable, "-m", "pip", "uninstall", "-y", "orjson"], 
                   capture_output=True, check=False)
    # Find the wheel file
    wheels = glob.glob("target/wheels/orjson*.whl")
    if wheels:
        subprocess.run([sys.executable, "-m", "pip", "install", "--user", wheels[0]], 
                      capture_output=True, check=False)
    else:
        print("ERROR: Could not find wheel file. Building...")
        subprocess.run([sys.executable, "-m", "maturin", "build", "--release"], 
                      check=False)
        wheels = glob.glob("target/wheels/orjson*.whl")
        if wheels:
            subprocess.run([sys.executable, "-m", "pip", "install", "--user", wheels[0]], 
                          capture_output=True, check=False)
    
    # Clear module cache and reimport
    if 'orjson' in sys.modules:
        del sys.modules['orjson']
    import orjson as orjson_modified
    
    result = benchmark_orjson(orjson_modified, "Modified orjson (subinterpreter-compatible)", 
                             test_data, iterations)
    results.append(result)
    
    # Comparison
    if len(results) == 2:
        print(f"\n{'='*60}")
        print("PERFORMANCE COMPARISON")
        print(f"{'='*60}")
        
        orig = results[0]
        mod = results[1]
        
        print(f"\nSerialization:")
        serialize_diff = ((mod['serialize_time'] - orig['serialize_time']) / orig['serialize_time']) * 100
        print(f"  Original:  {orig['serialize_ops_per_sec']:,.0f} ops/sec")
        print(f"  Modified:  {mod['serialize_ops_per_sec']:,.0f} ops/sec")
        if serialize_diff > 0:
            print(f"  Modified is {serialize_diff:.2f}% slower")
        else:
            print(f"  Modified is {abs(serialize_diff):.2f}% faster")
        
        print(f"\nDeserialization:")
        deserialize_diff = ((mod['deserialize_time'] - orig['deserialize_time']) / orig['deserialize_time']) * 100
        print(f"  Original:  {orig['deserialize_ops_per_sec']:,.0f} ops/sec")
        print(f"  Modified:  {mod['deserialize_ops_per_sec']:,.0f} ops/sec")
        if deserialize_diff > 0:
            print(f"  Modified is {deserialize_diff:.2f}% slower")
        else:
            print(f"  Modified is {abs(deserialize_diff):.2f}% faster")
        
        print(f"\nRound-trip:")
        roundtrip_diff = ((mod['roundtrip_time'] - orig['roundtrip_time']) / orig['roundtrip_time']) * 100
        print(f"  Original:  {orig['roundtrip_ops_per_sec']:,.0f} ops/sec")
        print(f"  Modified:  {mod['roundtrip_ops_per_sec']:,.0f} ops/sec")
        if roundtrip_diff > 0:
            print(f"  Modified is {roundtrip_diff:.2f}% slower")
        else:
            print(f"  Modified is {abs(roundtrip_diff):.2f}% faster")
        
        # Overall assessment
        max_diff = max(abs(serialize_diff), abs(deserialize_diff), abs(roundtrip_diff))
        avg_diff = (serialize_diff + deserialize_diff + roundtrip_diff) / 3
        
        print(f"\n{'='*60}")
        print(f"Overall Assessment:")
        print(f"  Average difference: {avg_diff:+.2f}%")
        print(f"  Maximum difference: {max_diff:.2f}%")
        
        if abs(avg_diff) < 2:
            print(f"\n✅ Performance is excellent - within 2% of original!")
        elif abs(avg_diff) < 5:
            print(f"\n✅ Performance is very good - within 5% of original!")
        elif abs(avg_diff) < 10:
            print(f"\n⚠️  Performance is acceptable - within 10% of original")
        else:
            if avg_diff > 0:
                print(f"\n❌ Performance is {avg_diff:.2f}% slower - may need optimization")
            else:
                print(f"\n✅ Performance is {abs(avg_diff):.2f}% faster - excellent improvement!")


if __name__ == "__main__":
    main()
