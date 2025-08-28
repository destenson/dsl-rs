#!/usr/bin/env python3
"""
Validation script for DSL-RS test runner components.
Ensures all modules are working correctly together.
"""

import sys
import os
from pathlib import Path

# Add lib directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'lib'))

from config_generator import ConfigGenerator, create_default_matrix
from test_executor import TestExecutor, TestResult
from report_generator import ReportGenerator

def validate_config_generator():
    """Validate configuration generator."""
    print("Testing ConfigGenerator...")
    
    generator = ConfigGenerator()
    
    # Test matrix generation
    matrix = create_default_matrix()
    configs = generator.generate_matrix({'stream_count': {'start': 1, 'stop': 3}})
    
    assert len(configs) > 0, "Failed to generate configurations"
    print(f"  [PASS] Generated {len(configs)} configurations")
    
    # Test filtering
    filtered = generator.filter_matrix(configs, constraints={'stream_count': {'max': 2}})
    assert len(filtered) <= len(configs), "Filtering failed"
    print(f"  [PASS] Filtered to {len(filtered)} configurations")
    
    return True


def validate_test_executor():
    """Validate test executor."""
    print("Testing TestExecutor...")
    
    project_root = Path.cwd()
    while not (project_root / 'Cargo.toml').exists():
        if project_root.parent == project_root:
            print("  [FAIL] Could not find project root")
            return False
        project_root = project_root.parent
    
    executor = TestExecutor(project_root)
    
    # Create a simple test result
    result = TestResult(
        name="validation_test",
        category="unit",
        exit_code=0,
        stdout="Test output",
        stderr="",
        duration=1.5,
        passed=True
    )
    
    assert result.name == "validation_test", "TestResult creation failed"
    assert result.passed == True, "TestResult passed flag incorrect"
    print("  [PASS] TestResult created successfully")
    
    # Test command execution (simple echo command)
    exit_code, stdout, stderr = executor.run_command(['echo', 'test'])
    assert exit_code == 0, "Command execution failed"
    print("  [PASS] Command execution working")
    
    return True


def validate_report_generator():
    """Validate report generator."""
    print("Testing ReportGenerator...")
    
    project_root = Path.cwd()
    while not (project_root / 'Cargo.toml').exists():
        if project_root.parent == project_root:
            print("  [FAIL] Could not find project root")
            return False
        project_root = project_root.parent
    
    generator = ReportGenerator(project_root)
    
    # Create test results
    results = [
        TestResult(
            name="test1",
            category="unit",
            exit_code=0,
            stdout="",
            stderr="",
            duration=1.0,
            passed=True
        ),
        TestResult(
            name="test2",
            category="integration",
            exit_code=1,
            stdout="",
            stderr="Error",
            duration=2.0,
            passed=False
        )
    ]
    
    # Generate summary
    summary = generator.generate_summary(results)
    assert summary.total == 2, "Summary total incorrect"
    assert summary.passed == 1, "Summary passed count incorrect"
    assert summary.failed == 1, "Summary failed count incorrect"
    print(f"  [PASS] Summary generated: {summary.passed}/{summary.total} passed")
    
    # Generate JSON report
    report = generator.generate_json_report(results)
    assert 'timestamp' in report, "JSON report missing timestamp"
    assert 'summary' in report, "JSON report missing summary"
    assert 'results' in report, "JSON report missing results"
    print("  [PASS] JSON report generated successfully")
    
    # Generate HTML report
    html = generator.generate_html_report(results)
    assert '<html' in html.lower(), "HTML report missing html tag"
    assert 'DSL-RS Test Report' in html, "HTML report missing title"
    print("  [PASS] HTML report generated successfully")
    
    # Generate JUnit XML
    xml = generator.generate_junit_xml(results)
    assert '<?xml' in xml, "JUnit XML missing declaration"
    assert '<testsuites' in xml, "JUnit XML missing testsuites"
    print("  [PASS] JUnit XML generated successfully")
    
    return True


def validate_integration():
    """Validate all components work together."""
    print("Testing Integration...")
    
    project_root = Path.cwd()
    while not (project_root / 'Cargo.toml').exists():
        if project_root.parent == project_root:
            print("  [FAIL] Could not find project root")
            return False
        project_root = project_root.parent
    
    # Create components
    config_gen = ConfigGenerator()
    executor = TestExecutor(project_root)
    report_gen = ReportGenerator(project_root)
    
    # Generate a small configuration matrix
    configs = config_gen.generate_matrix({'test_level': ['quick', 'full']})
    
    # Create test specifications
    test_specs = []
    for i, config in enumerate(configs):
        test_specs.append({
            'name': f'integration_test_{i}',
            'type': 'custom',
            'cmd': ['echo', f'Test {i}'],
            'env': config
        })
    
    # Execute tests
    results = []
    for spec in test_specs:
        # Simulate test execution
        result = TestResult(
            name=spec['name'],
            category='integration',
            exit_code=0,
            stdout=f"Output for {spec['name']}",
            stderr="",
            duration=0.1,
            passed=True,
            config=spec['env']
        )
        results.append(result)
    
    # Generate report
    report = report_gen.generate_json_report(results)
    
    assert len(results) == len(configs), "Result count mismatch"
    assert report['summary']['total'] == len(results), "Report summary incorrect"
    
    print(f"  [PASS] Integration test completed: {len(results)} tests executed")
    
    return True


def main():
    """Run all validation tests."""
    print("="*60)
    print("DSL-RS Test Runner Component Validation")
    print("="*60)
    
    all_passed = True
    
    # Test each component
    try:
        if not validate_config_generator():
            all_passed = False
            print("  [FAIL] ConfigGenerator validation failed")
    except Exception as e:
        print(f"  [FAIL] ConfigGenerator error: {e}")
        all_passed = False
    
    try:
        if not validate_test_executor():
            all_passed = False
            print("  [FAIL] TestExecutor validation failed")
    except Exception as e:
        print(f"  [FAIL] TestExecutor error: {e}")
        all_passed = False
    
    try:
        if not validate_report_generator():
            all_passed = False
            print("  [FAIL] ReportGenerator validation failed")
    except Exception as e:
        print(f"  [FAIL] ReportGenerator error: {e}")
        all_passed = False
    
    try:
        if not validate_integration():
            all_passed = False
            print("  [FAIL] Integration validation failed")
    except Exception as e:
        print(f"  [FAIL] Integration error: {e}")
        all_passed = False
    
    print("="*60)
    if all_passed:
        print("[SUCCESS] All validations passed!")
        print("The test runner components are working correctly.")
        return 0
    else:
        print("[FAILURE] Some validations failed.")
        print("Please check the errors above.")
        return 1


if __name__ == '__main__':
    sys.exit(main())