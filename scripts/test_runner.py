#!/usr/bin/env python3
"""
DSL-RS Test Runner
Orchestrates test execution with different configurations and generates reports.
"""

import sys
import os
import json
import subprocess
import argparse
import time
import concurrent.futures
from pathlib import Path
from datetime import datetime
from typing import List, Dict, Any, Tuple, Optional
import platform
import multiprocessing
import yaml

# Add lib directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'lib'))

from config_generator import ConfigGenerator
from test_executor import TestExecutor, TestResult
from report_generator import ReportGenerator

class TestRunner:
    def __init__(self, project_root: Path, config_file: Optional[Path] = None):
        self.project_root = project_root
        self.results = []
        self.start_time = None
        self.end_time = None
        
        # Initialize library components
        self.executor = TestExecutor(project_root)
        self.report_generator = ReportGenerator(project_root)
        
        # Load configuration if provided
        self.config = {}
        if config_file and config_file.exists():
            self.load_config(config_file)
        else:
            # Try to load default config
            default_config = project_root / 'scripts' / 'configs' / 'test_matrix.yaml'
            if default_config.exists():
                self.load_config(default_config)
    
    def load_config(self, config_file: Path):
        """Load configuration from YAML file."""
        with open(config_file, 'r') as f:
            self.config = yaml.safe_load(f)
        
    def run_command(self, cmd: List[str], env: Dict[str, str] = None) -> Tuple[int, str, str]:
        """Execute a command and return exit code, stdout, stderr."""
        env_vars = os.environ.copy()
        if env:
            env_vars.update(env)
        
        process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env_vars,
            cwd=self.project_root
        )
        stdout, stderr = process.communicate()
        return process.returncode, stdout.decode('utf-8'), stderr.decode('utf-8')
    
    def run_unit_tests(self) -> TestResult:
        """Run unit tests."""
        print("Running unit tests...")
        return self.executor.run_cargo_test(
            test_name='unit_tests',
            test_filter='--lib',
            timeout=self.config.get('timeouts', {}).get('unit', 300)
        )
    
    def run_integration_tests(self) -> TestResult:
        """Run integration tests."""
        print("Running integration tests...")
        return self.executor.run_cargo_test(
            test_name='integration_tests',
            test_filter='--test *',
            timeout=self.config.get('timeouts', {}).get('integration', 600)
        )
    
    def run_benchmarks(self) -> TestResult:
        """Run performance benchmarks."""
        print("Running benchmarks...")
        return self.executor.run_custom_command(
            name='benchmarks',
            cmd=['cargo', 'bench'],
            timeout=self.config.get('timeouts', {}).get('performance', 900)
        )
    
    def run_configuration_matrix(self) -> List[TestResult]:
        """Run tests with different configuration matrices."""
        print("Running configuration matrix tests...")
        
        # Use config generator to create matrix
        generator = ConfigGenerator()
        
        # Get configurations from config or use defaults
        if 'env_presets' in self.config:
            configurations = [
                {'name': name, 'env': env}
                for name, env in self.config['env_presets'].items()
            ]
        else:
            configurations = [
                {
                    'name': 'minimal',
                    'env': {
                        'WATCHDOG_TIMEOUT': '0',
                        'MAX_STREAMS': '1',
                        'ENABLE_METRICS': 'false'
                    }
                },
                {
                    'name': 'standard',
                    'env': {
                        'WATCHDOG_TIMEOUT': '60',
                        'MAX_STREAMS': '10',
                        'ENABLE_METRICS': 'true'
                    }
                },
                {
                    'name': 'maximum',
                    'env': {
                        'WATCHDOG_TIMEOUT': '300',
                        'MAX_STREAMS': '100',
                        'ENABLE_METRICS': 'true'
                    }
                }
            ]
        
        # Create test specifications
        test_specs = []
        for config in configurations:
            test_specs.append({
                'name': f"config_matrix_{config['name']}",
                'type': 'cargo_test',
                'filter': 'configurations',
                'env': config['env'],
                'timeout': 300
            })
        
        # Run tests with executor
        return self.executor.run_test_batch(test_specs, parallel=True)
    
    def run_chaos_tests(self) -> TestResult:
        """Run chaos engineering tests."""
        print("Running chaos tests...")
        return self.executor.run_cargo_test(
            test_name='chaos_tests',
            test_filter='chaos',
            timeout=self.config.get('timeouts', {}).get('chaos', 1200)
        )
    
    def run_parallel_tests(self, test_categories: List[str]) -> List[Dict[str, Any]]:
        """Run multiple test categories in parallel."""
        print(f"Running tests in parallel: {test_categories}")
        
        with concurrent.futures.ThreadPoolExecutor(max_workers=len(test_categories)) as executor:
            futures = {}
            
            for category in test_categories:
                if category == 'unit':
                    futures[executor.submit(self.run_unit_tests)] = category
                elif category == 'integration':
                    futures[executor.submit(self.run_integration_tests)] = category
                elif category == 'chaos':
                    futures[executor.submit(self.run_chaos_tests)] = category
            
            results = []
            for future in concurrent.futures.as_completed(futures):
                try:
                    result = future.result()
                    results.append(result)
                except Exception as e:
                    print(f"Error running {futures[future]}: {e}")
            
            return results
    
    def generate_report(self) -> Dict[str, Any]:
        """Generate test report using ReportGenerator."""
        total_duration = (self.end_time - self.start_time) if self.end_time else 0
        
        # Add duration to metadata
        metadata = {
            'total_duration': total_duration,
            'config_file': str(self.config.get('name', 'default'))
        }
        
        return self.report_generator.generate_json_report(self.results, metadata)
    
    def save_report(self, format: str = 'json'):
        """Save test report to file."""
        timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
        
        if format == 'json':
            filename = f'report_{timestamp}.json'
            self.report_generator.save_json_report(self.results, filename)
        
        elif format == 'html':
            filename = f'report_{timestamp}.html'
            self.report_generator.save_html_report(self.results, filename)
        
        elif format == 'junit':
            filename = f'junit_{timestamp}.xml'
            self.report_generator.save_junit_xml(self.results, filename)
    
    
    def run(self, args):
        """Main test execution."""
        self.start_time = time.time()
        
        if args.parallel:
            self.results.extend(self.run_parallel_tests(['unit', 'integration', 'chaos']))
        else:
            if args.unit or args.all:
                self.results.append(self.run_unit_tests())
            
            if args.integration or args.all:
                self.results.append(self.run_integration_tests())
            
            if args.benchmarks:
                self.results.append(self.run_benchmarks())
            
            if args.matrix or args.all:
                self.results.extend(self.run_configuration_matrix())
            
            if args.chaos or args.all:
                self.results.append(self.run_chaos_tests())
        
        self.end_time = time.time()
        
        # Generate and save report
        report = self.generate_report()
        
        if args.json:
            self.save_report('json')
        
        if args.html:
            self.save_report('html')
            
        if hasattr(args, 'junit') and args.junit:
            self.save_report('junit')
        
        # Calculate summary stats
        summary = self.report_generator.generate_summary(self.results)
        
        # Print summary
        print("\n" + "="*50)
        print("TEST SUMMARY")
        print("="*50)
        print(f"Total Duration: {report['summary']['duration']:.2f} seconds")
        print(f"Total Tests: {summary.total}")
        print(f"Passed: {summary.passed}")
        print(f"Failed: {summary.failed}")
        print(f"Errors: {summary.errors}")
        print(f"Success Rate: {summary.success_rate:.1f}%")
        
        # Return non-zero exit code if any tests failed
        return 0 if summary.failed == 0 and summary.errors == 0 else 1


def main():
    parser = argparse.ArgumentParser(description='DSL-RS Test Runner')
    parser.add_argument('--all', action='store_true', help='Run all tests')
    parser.add_argument('--unit', action='store_true', help='Run unit tests')
    parser.add_argument('--integration', action='store_true', help='Run integration tests')
    parser.add_argument('--benchmarks', action='store_true', help='Run benchmarks')
    parser.add_argument('--matrix', action='store_true', help='Run configuration matrix tests')
    parser.add_argument('--chaos', action='store_true', help='Run chaos tests')
    parser.add_argument('--parallel', action='store_true', help='Run tests in parallel')
    parser.add_argument('--json', action='store_true', help='Generate JSON report')
    parser.add_argument('--html', action='store_true', help='Generate HTML report')
    parser.add_argument('--junit', action='store_true', help='Generate JUnit XML report')
    parser.add_argument('--config', type=str, help='Path to configuration file')
    parser.add_argument('--scenario', type=str, help='Run specific test scenario from configs/scenarios/')
    
    args = parser.parse_args()
    
    # Default to all tests if none specified
    if not any([args.all, args.unit, args.integration, args.benchmarks, args.matrix, args.chaos]):
        args.all = True
    
    # Find project root
    project_root = Path.cwd()
    while not (project_root / 'Cargo.toml').exists():
        if project_root.parent == project_root:
            print("Error: Could not find Cargo.toml")
            sys.exit(1)
        project_root = project_root.parent
    
    # Load configuration if specified
    config_file = None
    if args.config:
        config_file = Path(args.config)
        if not config_file.exists():
            print(f"Error: Config file not found: {config_file}")
            sys.exit(1)
    elif args.scenario:
        # Load scenario configuration
        scenario_file = project_root / 'scripts' / 'configs' / 'scenarios' / f'{args.scenario}.yaml'
        if scenario_file.exists():
            config_file = scenario_file
        else:
            print(f"Error: Scenario file not found: {scenario_file}")
            sys.exit(1)
    
    runner = TestRunner(project_root, config_file)
    sys.exit(runner.run(args))


if __name__ == '__main__':
    main()