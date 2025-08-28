"""
Report generator for DSL-RS test runner.
Generates HTML and JSON reports with detailed test results.
"""

import json
import html
from pathlib import Path
from datetime import datetime
from typing import List, Dict, Any, Optional
from dataclasses import dataclass, asdict
import platform
import sys


@dataclass
class TestSummary:
    """Summary statistics for test results."""
    total: int
    passed: int
    failed: int
    errors: int
    skipped: int
    duration: float
    success_rate: float
    categories: Dict[str, Dict[str, int]]


class ReportGenerator:
    """Generate test reports in various formats."""
    
    def __init__(self, project_root: Path, report_dir: Optional[Path] = None):
        """
        Initialize report generator.
        
        Args:
            project_root: Root directory of the project
            report_dir: Directory to save reports (defaults to project_root/test-reports)
        """
        self.project_root = project_root
        self.report_dir = report_dir or project_root / 'test-reports'
        self.report_dir.mkdir(parents=True, exist_ok=True)
        
    def generate_summary(self, results: List[Any]) -> TestSummary:
        """
        Generate summary statistics from test results.
        
        Args:
            results: List of test results (TestResult objects or dicts)
        
        Returns:
            TestSummary object
        """
        total = len(results)
        passed = sum(1 for r in results if self._get_field(r, 'passed'))
        failed = sum(1 for r in results if not self._get_field(r, 'passed') and not self._get_field(r, 'error'))
        errors = sum(1 for r in results if self._get_field(r, 'error'))
        skipped = sum(1 for r in results if self._get_field(r, 'skipped', False))
        duration = sum(self._get_field(r, 'duration', 0) for r in results)
        
        # Calculate success rate
        success_rate = (passed / total * 100) if total > 0 else 0
        
        # Group by categories
        categories = {}
        for result in results:
            category = self._get_field(result, 'category', 'unknown')
            if category not in categories:
                categories[category] = {'total': 0, 'passed': 0, 'failed': 0}
            
            categories[category]['total'] += 1
            if self._get_field(result, 'passed'):
                categories[category]['passed'] += 1
            else:
                categories[category]['failed'] += 1
        
        return TestSummary(
            total=total,
            passed=passed,
            failed=failed,
            errors=errors,
            skipped=skipped,
            duration=duration,
            success_rate=success_rate,
            categories=categories
        )
    
    def _get_field(self, obj: Any, field: str, default: Any = None) -> Any:
        """Helper to get field from object or dict."""
        if hasattr(obj, field):
            return getattr(obj, field)
        elif isinstance(obj, dict):
            return obj.get(field, default)
        return default
    
    def generate_json_report(self, 
                           results: List[Any],
                           metadata: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """
        Generate JSON report.
        
        Args:
            results: List of test results
            metadata: Optional metadata to include
        
        Returns:
            Dictionary containing the full report
        """
        summary = self.generate_summary(results)
        
        # Convert results to dictionaries
        results_data = []
        for r in results:
            if hasattr(r, '__dict__'):
                data = {k: v for k, v in r.__dict__.items() if not k.startswith('_')}
                # Convert datetime objects to strings
                for k, v in data.items():
                    if isinstance(v, datetime):
                        data[k] = v.isoformat()
                results_data.append(data)
            else:
                results_data.append(r)
        
        report = {
            'timestamp': datetime.now().isoformat(),
            'platform': {
                'system': platform.system(),
                'release': platform.release(),
                'version': platform.version(),
                'machine': platform.machine(),
                'processor': platform.processor(),
                'python_version': platform.python_version()
            },
            'summary': asdict(summary),
            'metadata': metadata or {},
            'results': results_data
        }
        
        return report
    
    def save_json_report(self, 
                        results: List[Any],
                        filename: Optional[str] = None,
                        metadata: Optional[Dict[str, Any]] = None) -> Path:
        """
        Save JSON report to file.
        
        Args:
            results: List of test results
            filename: Optional filename (defaults to timestamp-based name)
            metadata: Optional metadata to include
        
        Returns:
            Path to saved report
        """
        if not filename:
            timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
            filename = f'report_{timestamp}.json'
        
        report_path = self.report_dir / filename
        report = self.generate_json_report(results, metadata)
        
        with open(report_path, 'w') as f:
            json.dump(report, f, indent=2)
        
        print(f"JSON report saved to {report_path}")
        return report_path
    
    def generate_html_report(self,
                           results: List[Any],
                           metadata: Optional[Dict[str, Any]] = None) -> str:
        """
        Generate HTML report.
        
        Args:
            results: List of test results
            metadata: Optional metadata to include
        
        Returns:
            HTML string
        """
        summary = self.generate_summary(results)
        report_data = self.generate_json_report(results, metadata)
        
        # Generate HTML
        html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>DSL-RS Test Report</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            line-height: 1.6;
            color: #333;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
            padding: 20px;
        }}
        
        .container {{
            max-width: 1400px;
            margin: 0 auto;
            background: white;
            border-radius: 10px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
            overflow: hidden;
        }}
        
        header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 30px;
            text-align: center;
        }}
        
        h1 {{
            font-size: 2.5em;
            margin-bottom: 10px;
        }}
        
        .timestamp {{
            opacity: 0.9;
            font-size: 0.9em;
        }}
        
        .summary {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            padding: 30px;
            background: #f8f9fa;
        }}
        
        .summary-card {{
            background: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            text-align: center;
        }}
        
        .summary-card h3 {{
            color: #666;
            font-size: 0.9em;
            text-transform: uppercase;
            margin-bottom: 10px;
        }}
        
        .summary-card .value {{
            font-size: 2em;
            font-weight: bold;
            color: #333;
        }}
        
        .summary-card.passed .value {{
            color: #28a745;
        }}
        
        .summary-card.failed .value {{
            color: #dc3545;
        }}
        
        .summary-card.errors .value {{
            color: #ff6b6b;
        }}
        
        .content {{
            padding: 30px;
        }}
        
        .section {{
            margin-bottom: 40px;
        }}
        
        h2 {{
            color: #333;
            margin-bottom: 20px;
            padding-bottom: 10px;
            border-bottom: 2px solid #667eea;
        }}
        
        .categories {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }}
        
        .category-card {{
            background: white;
            border: 1px solid #e0e0e0;
            border-radius: 8px;
            padding: 15px;
        }}
        
        .category-card h4 {{
            margin-bottom: 10px;
            color: #666;
        }}
        
        .progress-bar {{
            height: 20px;
            background: #e0e0e0;
            border-radius: 10px;
            overflow: hidden;
            margin-bottom: 10px;
        }}
        
        .progress-fill {{
            height: 100%;
            background: linear-gradient(90deg, #28a745 0%, #20c997 100%);
            transition: width 0.3s;
        }}
        
        table {{
            width: 100%;
            border-collapse: collapse;
            margin-top: 20px;
        }}
        
        th {{
            background: #667eea;
            color: white;
            padding: 12px;
            text-align: left;
            font-weight: 600;
        }}
        
        td {{
            padding: 12px;
            border-bottom: 1px solid #e0e0e0;
        }}
        
        tr:hover {{
            background: #f8f9fa;
        }}
        
        .status {{
            display: inline-block;
            padding: 4px 12px;
            border-radius: 20px;
            font-size: 0.85em;
            font-weight: 600;
        }}
        
        .status.passed {{
            background: #d4edda;
            color: #155724;
        }}
        
        .status.failed {{
            background: #f8d7da;
            color: #721c24;
        }}
        
        .status.error {{
            background: #fff3cd;
            color: #856404;
        }}
        
        .details {{
            max-width: 300px;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }}
        
        .details:hover {{
            overflow: visible;
            white-space: normal;
            background: white;
            position: relative;
            z-index: 10;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            padding: 5px;
        }}
        
        .platform-info {{
            background: #f8f9fa;
            padding: 15px;
            border-radius: 8px;
            margin-bottom: 20px;
        }}
        
        .platform-info span {{
            display: inline-block;
            margin-right: 20px;
            color: #666;
        }}
        
        .platform-info strong {{
            color: #333;
        }}
        
        @media (max-width: 768px) {{
            .summary {{
                grid-template-columns: 1fr;
            }}
            
            h1 {{
                font-size: 1.8em;
            }}
            
            table {{
                font-size: 0.9em;
            }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>DSL-RS Test Report</h1>
            <p class="timestamp">Generated on {report_data['timestamp']}</p>
        </header>
        
        <div class="summary">
            <div class="summary-card">
                <h3>Total Tests</h3>
                <div class="value">{summary.total}</div>
            </div>
            <div class="summary-card passed">
                <h3>Passed</h3>
                <div class="value">{summary.passed}</div>
            </div>
            <div class="summary-card failed">
                <h3>Failed</h3>
                <div class="value">{summary.failed}</div>
            </div>
            <div class="summary-card errors">
                <h3>Errors</h3>
                <div class="value">{summary.errors}</div>
            </div>
            <div class="summary-card">
                <h3>Success Rate</h3>
                <div class="value">{summary.success_rate:.1f}%</div>
            </div>
            <div class="summary-card">
                <h3>Total Duration</h3>
                <div class="value">{summary.duration:.2f}s</div>
            </div>
        </div>
        
        <div class="content">
            <div class="section">
                <h2>Platform Information</h2>
                <div class="platform-info">
                    <span><strong>System:</strong> {report_data['platform']['system']}</span>
                    <span><strong>Python:</strong> {report_data['platform']['python_version']}</span>
                    <span><strong>Machine:</strong> {report_data['platform']['machine']}</span>
                    <span><strong>Processor:</strong> {report_data['platform']['processor']}</span>
                </div>
            </div>
            
            <div class="section">
                <h2>Test Categories</h2>
                <div class="categories">
"""
        
        # Add category cards
        for category, stats in summary.categories.items():
            success_rate = (stats['passed'] / stats['total'] * 100) if stats['total'] > 0 else 0
            html_content += f"""
                    <div class="category-card">
                        <h4>{category}</h4>
                        <div class="progress-bar">
                            <div class="progress-fill" style="width: {success_rate}%"></div>
                        </div>
                        <p>{stats['passed']}/{stats['total']} passed ({success_rate:.1f}%)</p>
                    </div>
"""
        
        html_content += """
                </div>
            </div>
            
            <div class="section">
                <h2>Test Results</h2>
                <table id="resultsTable">
                    <thead>
                        <tr>
                            <th>Test Name</th>
                            <th>Category</th>
                            <th>Status</th>
                            <th>Duration</th>
                            <th>Details</th>
                        </tr>
                    </thead>
                    <tbody>
"""
        
        # Add test results
        for result in results:
            name = html.escape(str(self._get_field(result, 'name', 'Unknown')))
            category = html.escape(str(self._get_field(result, 'category', 'Unknown')))
            passed = self._get_field(result, 'passed', False)
            error = self._get_field(result, 'error')
            duration = self._get_field(result, 'duration', 0)
            
            if error:
                status = 'error'
                status_text = 'ERROR'
                details = html.escape(str(error))
            elif passed:
                status = 'passed'
                status_text = 'PASSED'
                details = 'Test completed successfully'
            else:
                status = 'failed'
                status_text = 'FAILED'
                stderr = self._get_field(result, 'stderr', '')
                details = html.escape(stderr[:200] if stderr else 'Test failed')
            
            html_content += f"""
                        <tr>
                            <td>{name}</td>
                            <td>{category}</td>
                            <td><span class="status {status}">{status_text}</span></td>
                            <td>{duration:.2f}s</td>
                            <td class="details" title="{details}">{details}</td>
                        </tr>
"""
        
        html_content += """
                    </tbody>
                </table>
            </div>
        </div>
    </div>
    
    <script>
        // Add sorting functionality to the table
        document.addEventListener('DOMContentLoaded', function() {
            const table = document.getElementById('resultsTable');
            const headers = table.querySelectorAll('th');
            
            headers.forEach((header, index) => {
                header.style.cursor = 'pointer';
                header.addEventListener('click', () => {
                    sortTable(table, index);
                });
            });
        });
        
        function sortTable(table, column) {
            const tbody = table.querySelector('tbody');
            const rows = Array.from(tbody.querySelectorAll('tr'));
            
            rows.sort((a, b) => {
                const aValue = a.children[column].textContent;
                const bValue = b.children[column].textContent;
                
                // Try to parse as number first
                const aNum = parseFloat(aValue);
                const bNum = parseFloat(bValue);
                
                if (!isNaN(aNum) && !isNaN(bNum)) {
                    return aNum - bNum;
                }
                
                return aValue.localeCompare(bValue);
            });
            
            tbody.innerHTML = '';
            rows.forEach(row => tbody.appendChild(row));
        }
    </script>
</body>
</html>
"""
        
        return html_content
    
    def save_html_report(self,
                        results: List[Any],
                        filename: Optional[str] = None,
                        metadata: Optional[Dict[str, Any]] = None) -> Path:
        """
        Save HTML report to file.
        
        Args:
            results: List of test results
            filename: Optional filename (defaults to timestamp-based name)
            metadata: Optional metadata to include
        
        Returns:
            Path to saved report
        """
        if not filename:
            timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
            filename = f'report_{timestamp}.html'
        
        report_path = self.report_dir / filename
        html_content = self.generate_html_report(results, metadata)
        
        with open(report_path, 'w', encoding='utf-8') as f:
            f.write(html_content)
        
        print(f"HTML report saved to {report_path}")
        return report_path
    
    def generate_junit_xml(self, results: List[Any]) -> str:
        """
        Generate JUnit XML report for CI/CD integration.
        
        Args:
            results: List of test results
        
        Returns:
            XML string
        """
        summary = self.generate_summary(results)
        timestamp = datetime.now().isoformat()
        
        xml_content = f"""<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="DSL-RS Tests" tests="{summary.total}" failures="{summary.failed}" errors="{summary.errors}" time="{summary.duration:.3f}">
    <testsuite name="DSL-RS" tests="{summary.total}" failures="{summary.failed}" errors="{summary.errors}" time="{summary.duration:.3f}" timestamp="{timestamp}">
"""
        
        for result in results:
            name = html.escape(str(self._get_field(result, 'name', 'Unknown')))
            category = html.escape(str(self._get_field(result, 'category', 'Unknown')))
            duration = self._get_field(result, 'duration', 0)
            passed = self._get_field(result, 'passed', False)
            error = self._get_field(result, 'error')
            
            xml_content += f'        <testcase classname="{category}" name="{name}" time="{duration:.3f}">\n'
            
            if error:
                error_msg = html.escape(str(error))
                xml_content += f'            <error message="{error_msg}"/>\n'
            elif not passed:
                stderr = html.escape(str(self._get_field(result, 'stderr', 'Test failed')))
                xml_content += f'            <failure message="Test failed">{stderr}</failure>\n'
            
            xml_content += '        </testcase>\n'
        
        xml_content += """    </testsuite>
</testsuites>
"""
        
        return xml_content
    
    def save_junit_xml(self,
                      results: List[Any],
                      filename: Optional[str] = None) -> Path:
        """
        Save JUnit XML report.
        
        Args:
            results: List of test results
            filename: Optional filename
        
        Returns:
            Path to saved report
        """
        if not filename:
            timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
            filename = f'junit_{timestamp}.xml'
        
        report_path = self.report_dir / filename
        xml_content = self.generate_junit_xml(results)
        
        with open(report_path, 'w', encoding='utf-8') as f:
            f.write(xml_content)
        
        print(f"JUnit XML report saved to {report_path}")
        return report_path


if __name__ == '__main__':
    # Example usage
    from pathlib import Path
    
    # Create sample test results
    sample_results = [
        {
            'name': 'test_pipeline_creation',
            'category': 'unit',
            'passed': True,
            'duration': 0.5,
            'exit_code': 0
        },
        {
            'name': 'test_stream_manager',
            'category': 'unit',
            'passed': True,
            'duration': 1.2,
            'exit_code': 0
        },
        {
            'name': 'test_recovery_circuit_breaker',
            'category': 'integration',
            'passed': False,
            'duration': 3.4,
            'exit_code': 1,
            'stderr': 'Assertion failed: Circuit breaker did not trip'
        },
        {
            'name': 'test_chaos_network',
            'category': 'chaos',
            'passed': True,
            'duration': 10.5,
            'exit_code': 0
        }
    ]
    
    # Generate reports
    generator = ReportGenerator(Path.cwd())
    
    # Generate and save all report formats
    generator.save_json_report(sample_results)
    generator.save_html_report(sample_results)
    generator.save_junit_xml(sample_results)
    
    # Print summary
    summary = generator.generate_summary(sample_results)
    print(f"\nTest Summary: {summary.passed}/{summary.total} passed ({summary.success_rate:.1f}%)")