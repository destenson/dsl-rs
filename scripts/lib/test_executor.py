"""
Test executor for DSL-RS test runner.
Handles parallel execution, timeouts, and output capture.
"""

import subprocess
import time
import threading
import queue
import os
import signal
import platform
from pathlib import Path
from typing import Dict, List, Any, Tuple, Optional, Callable
from concurrent.futures import ThreadPoolExecutor, ProcessPoolExecutor, TimeoutError, as_completed
from dataclasses import dataclass, field
from datetime import datetime
import multiprocessing


@dataclass
class TestResult:
    """Container for test execution results."""
    name: str
    category: str
    exit_code: int
    stdout: str
    stderr: str
    duration: float
    passed: bool
    config: Dict[str, Any] = field(default_factory=dict)
    error: Optional[str] = None
    timestamp: datetime = field(default_factory=datetime.now)


class TestExecutor:
    """Execute tests with parallel support and resource management."""
    
    def __init__(self, project_root: Path, max_workers: Optional[int] = None):
        """
        Initialize test executor.
        
        Args:
            project_root: Root directory of the project
            max_workers: Maximum number of parallel workers (defaults to CPU count)
        """
        self.project_root = project_root
        self.max_workers = max_workers or multiprocessing.cpu_count()
        self.results_queue = queue.Queue()
        self.active_processes = {}
        self._shutdown = False
    
    def run_command(self, 
                   cmd: List[str], 
                   env: Optional[Dict[str, str]] = None,
                   timeout: Optional[int] = None,
                   cwd: Optional[Path] = None) -> Tuple[int, str, str]:
        """
        Execute a command with timeout and capture output.
        
        Args:
            cmd: Command to execute as list of strings
            env: Optional environment variables
            timeout: Optional timeout in seconds
            cwd: Optional working directory
        
        Returns:
            Tuple of (exit_code, stdout, stderr)
        """
        env_vars = os.environ.copy()
        if env:
            env_vars.update({k: str(v) for k, v in env.items()})
        
        working_dir = cwd or self.project_root
        
        try:
            process = subprocess.Popen(
                cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=env_vars,
                cwd=working_dir,
                universal_newlines=True
            )
            
            # Store process for potential cleanup
            self.active_processes[process.pid] = process
            
            try:
                stdout, stderr = process.communicate(timeout=timeout)
                exit_code = process.returncode
            except subprocess.TimeoutExpired:
                # Kill the process on timeout
                if platform.system() == 'Windows':
                    process.terminate()
                else:
                    process.send_signal(signal.SIGTERM)
                    time.sleep(1)
                    if process.poll() is None:
                        process.kill()
                
                stdout, stderr = process.communicate()
                exit_code = -1
                stderr = f"TIMEOUT: Process killed after {timeout} seconds\n{stderr}"
            finally:
                # Remove from active processes
                self.active_processes.pop(process.pid, None)
            
            return exit_code, stdout, stderr
            
        except Exception as e:
            return -1, "", f"Failed to execute command: {e}"
    
    def run_cargo_test(self,
                      test_name: str,
                      test_filter: Optional[str] = None,
                      features: Optional[List[str]] = None,
                      env: Optional[Dict[str, str]] = None,
                      timeout: Optional[int] = 300) -> TestResult:
        """
        Run a cargo test with specified parameters.
        
        Args:
            test_name: Name for this test run
            test_filter: Optional filter for test names
            features: Optional list of cargo features to enable
            env: Optional environment variables
            timeout: Timeout in seconds
        
        Returns:
            TestResult object
        """
        start_time = time.time()
        
        # Build cargo command
        cmd = ['cargo', 'test']
        
        if features:
            cmd.extend(['--features', ','.join(features)])
        
        if test_filter:
            cmd.append(test_filter)
        
        cmd.extend(['--', '--nocapture'])
        
        # Execute command
        exit_code, stdout, stderr = self.run_command(cmd, env=env, timeout=timeout)
        
        duration = time.time() - start_time
        
        return TestResult(
            name=test_name,
            category='cargo_test',
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            duration=duration,
            passed=(exit_code == 0),
            config={'filter': test_filter, 'features': features, 'env': env}
        )
    
    def run_cargo_example(self,
                         example_name: str,
                         args: Optional[List[str]] = None,
                         env: Optional[Dict[str, str]] = None,
                         timeout: Optional[int] = 300) -> TestResult:
        """
        Run a cargo example with specified parameters.
        
        Args:
            example_name: Name of the example to run
            args: Optional arguments to pass to the example
            env: Optional environment variables
            timeout: Timeout in seconds
        
        Returns:
            TestResult object
        """
        start_time = time.time()
        
        # Build cargo command
        cmd = ['cargo', 'run', '--example', example_name]
        
        if args:
            cmd.append('--')
            cmd.extend(args)
        
        # Execute command
        exit_code, stdout, stderr = self.run_command(cmd, env=env, timeout=timeout)
        
        duration = time.time() - start_time
        
        return TestResult(
            name=f"example_{example_name}",
            category='example',
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            duration=duration,
            passed=(exit_code == 0),
            config={'args': args, 'env': env}
        )
    
    def run_custom_command(self,
                          name: str,
                          cmd: List[str],
                          env: Optional[Dict[str, str]] = None,
                          timeout: Optional[int] = 300,
                          cwd: Optional[Path] = None) -> TestResult:
        """
        Run a custom command.
        
        Args:
            name: Name for this test
            cmd: Command to execute
            env: Optional environment variables
            timeout: Timeout in seconds
            cwd: Optional working directory
        
        Returns:
            TestResult object
        """
        start_time = time.time()
        
        exit_code, stdout, stderr = self.run_command(cmd, env=env, timeout=timeout, cwd=cwd)
        
        duration = time.time() - start_time
        
        return TestResult(
            name=name,
            category='custom',
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
            duration=duration,
            passed=(exit_code == 0),
            config={'cmd': cmd, 'env': env}
        )
    
    def run_test_batch(self, tests: List[Dict[str, Any]], parallel: bool = True) -> List[TestResult]:
        """
        Run a batch of tests, optionally in parallel.
        
        Args:
            tests: List of test specifications
            parallel: Whether to run tests in parallel
        
        Returns:
            List of TestResult objects
        """
        results = []
        
        if not parallel:
            # Sequential execution
            for test_spec in tests:
                result = self._execute_single_test(test_spec)
                results.append(result)
                self._print_progress(test_spec['name'], result.passed)
        else:
            # Parallel execution
            with ThreadPoolExecutor(max_workers=self.max_workers) as executor:
                future_to_test = {
                    executor.submit(self._execute_single_test, test_spec): test_spec
                    for test_spec in tests
                }
                
                for future in as_completed(future_to_test):
                    test_spec = future_to_test[future]
                    try:
                        result = future.result()
                        results.append(result)
                        self._print_progress(test_spec['name'], result.passed)
                    except Exception as e:
                        results.append(TestResult(
                            name=test_spec['name'],
                            category=test_spec.get('type', 'unknown'),
                            exit_code=-1,
                            stdout="",
                            stderr=str(e),
                            duration=0,
                            passed=False,
                            error=str(e)
                        ))
                        self._print_progress(test_spec['name'], False, error=True)
        
        return results
    
    def _execute_single_test(self, test_spec: Dict[str, Any]) -> TestResult:
        """Execute a single test based on specification."""
        test_type = test_spec.get('type', 'cargo_test')
        
        if test_type == 'cargo_test':
            return self.run_cargo_test(
                test_name=test_spec['name'],
                test_filter=test_spec.get('filter'),
                features=test_spec.get('features'),
                env=test_spec.get('env'),
                timeout=test_spec.get('timeout', 300)
            )
        elif test_type == 'example':
            return self.run_cargo_example(
                example_name=test_spec['example'],
                args=test_spec.get('args'),
                env=test_spec.get('env'),
                timeout=test_spec.get('timeout', 300)
            )
        elif test_type == 'custom':
            return self.run_custom_command(
                name=test_spec['name'],
                cmd=test_spec['cmd'],
                env=test_spec.get('env'),
                timeout=test_spec.get('timeout', 300),
                cwd=test_spec.get('cwd')
            )
        else:
            raise ValueError(f"Unknown test type: {test_type}")
    
    def _print_progress(self, test_name: str, passed: bool, error: bool = False):
        """Print test progress."""
        if error:
            status = "ERROR"
            symbol = "✗"
        elif passed:
            status = "PASS"
            symbol = "✓"
        else:
            status = "FAIL"
            symbol = "✗"
        
        print(f"  [{symbol}] {test_name}: {status}")
    
    def run_with_matrix(self, 
                       test_template: Dict[str, Any],
                       configurations: List[Dict[str, Any]],
                       parallel: bool = True) -> List[TestResult]:
        """
        Run tests with multiple configurations.
        
        Args:
            test_template: Base test specification
            configurations: List of configuration dictionaries
            parallel: Whether to run tests in parallel
        
        Returns:
            List of TestResult objects
        """
        tests = []
        for i, config in enumerate(configurations):
            test_spec = test_template.copy()
            test_spec['name'] = f"{test_template['name']}_config_{i}"
            test_spec['env'] = {**test_template.get('env', {}), **config}
            tests.append(test_spec)
        
        return self.run_test_batch(tests, parallel=parallel)
    
    def cleanup(self):
        """Clean up any remaining processes."""
        self._shutdown = True
        for pid, process in list(self.active_processes.items()):
            try:
                if platform.system() == 'Windows':
                    process.terminate()
                else:
                    process.send_signal(signal.SIGTERM)
            except:
                pass


def create_test_specifications(project_root: Path) -> List[Dict[str, Any]]:
    """Create default test specifications."""
    return [
        {
            'name': 'unit_tests',
            'type': 'cargo_test',
            'filter': '--lib',
            'timeout': 300
        },
        {
            'name': 'integration_tests',
            'type': 'cargo_test',
            'filter': '--test',
            'timeout': 600
        },
        {
            'name': 'doc_tests',
            'type': 'cargo_test',
            'filter': '--doc',
            'timeout': 300
        },
        {
            'name': 'example_robust_multistream',
            'type': 'example',
            'example': 'robust_multistream',
            'args': ['--duration', '10'],
            'timeout': 60
        }
    ]


if __name__ == '__main__':
    # Example usage
    project_root = Path.cwd()
    while not (project_root / 'Cargo.toml').exists():
        if project_root.parent == project_root:
            print("Error: Could not find Cargo.toml")
            exit(1)
        project_root = project_root.parent
    
    executor = TestExecutor(project_root)
    
    # Run some example tests
    print("Running test executor examples...")
    
    # Run unit tests
    result = executor.run_cargo_test("unit_tests", test_filter="--lib")
    print(f"Unit tests: {'PASSED' if result.passed else 'FAILED'} in {result.duration:.2f}s")
    
    # Run tests with different configurations
    configs = [
        {'RUST_BACKTRACE': '1', 'MAX_STREAMS': '10'},
        {'RUST_BACKTRACE': 'full', 'MAX_STREAMS': '100'},
    ]
    
    test_template = {
        'name': 'config_test',
        'type': 'cargo_test',
        'filter': 'configurations'
    }
    
    results = executor.run_with_matrix(test_template, configs)
    print(f"Matrix tests: {len([r for r in results if r.passed])}/{len(results)} passed")
    
    executor.cleanup()