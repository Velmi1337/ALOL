#!/usr/bin/env python3
"""Compare worker CPU cost on an idle synthetic mod tree. This is NOT game FPS.

Uses temporary profiles and fake Wine/UMU; no actual game or personal data.
Not a timing-sensitive unit test. Run explicitly with the prior launcher binary.
"""
import argparse
import json
import os
from pathlib import Path
import statistics
import signal
import subprocess
import time
import integration as base
from game_integration import DirectGame


def benchmark(baseline, candidate, files, seconds, runs):
    base.BINARY = candidate
    case = DirectGame()
    case.setUp()
    try:
        for directory in range(40):
            parent=case.game/'modloader'/f'pack-{directory}'
            parent.mkdir(parents=True)
            for number in range(directory, files, 40):
                (parent/f'asset-{number}.txd').touch()
        env=dict(case.env, ALOL_TEST_SLEEP=str(case.exe), ALOL_TEST_SLEEP_SECONDS=str(seconds+2))
        records=[]
        for run in range(runs):
            # Alternate order so one version does not always get the warm cache.
            order=[('before',baseline),('after',candidate)]
            if run % 2:order.reverse()
            for name, executable in order:
                log=case.root/'benchmark.log'
                ready=Path(env['ALOL_TEST_READY']);ready.unlink(missing_ok=True)
                with log.open('wb') as output:
                    child=subprocess.Popen([str(executable),'play'],env=env,stdout=output,stderr=output,start_new_session=True)
                    started=time.monotonic()
                    try:
                        deadline=started+5
                        while not ready.is_file():
                            if time.monotonic()>deadline or child.poll() is not None:
                                raise RuntimeError('Benchmark runner did not become ready')
                            time.sleep(.02)
                        # Exclude one-off initialization and the final cleanup.
                        time.sleep(.4)
                        scheduler=Path(f'/proc/{child.pid}/schedstat')
                        idle_before=int(scheduler.read_text().split()[0])
                        time.sleep(seconds)
                        idle_after=int(scheduler.read_text().split()[0])
                        _,status,usage=os.wait4(child.pid,0)
                    except BaseException:
                        os.killpg(child.pid,signal.SIGTERM)
                        try:child.wait(timeout=3)
                        except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
                        raise
                    child.returncode=os.waitstatus_to_exitcode(status)
                    elapsed=time.monotonic()-started
                if child.returncode:
                    raise RuntimeError(log.read_text(errors='replace'))
                record={'version':name,'run':run+1,'wall_seconds':round(elapsed,3),
                        'cpu_ms':round((usage.ru_utime+usage.ru_stime)*1000,3),
                        'idle_worker_cpu_ms':round((idle_after-idle_before)/1_000_000,3)}
                records.append(record)
                print(json.dumps(record),flush=True)
        medians={name:round(statistics.median(r['cpu_ms'] for r in records if r['version']==name),3)
                 for name in ('before','after')}
        idle_medians={name:round(statistics.median(r['idle_worker_cpu_ms'] for r in records if r['version']==name),3)
                      for name in ('before','after')}
        return {'kind':'synthetic_idle_worker_cpu_not_game_fps','files':files,'seconds':seconds,
                'runs_per_version':runs,'median_cpu_ms':medians,
                'median_idle_worker_cpu_ms':idle_medians,'records':records}
    finally:
        case.tearDown()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline',type=Path,required=True)
    parser.add_argument('--candidate',type=Path,default=base.BINARY)
    parser.add_argument('--files',type=int,default=40000)
    parser.add_argument('--seconds',type=float,default=6)
    parser.add_argument('--runs',type=int,default=3)
    parser.add_argument('--output',type=Path)
    args=parser.parse_args()
    if not (0 <= args.files <= 100000 and 3 <= args.seconds <= 30 and 1 <= args.runs <= 10):
        parser.error('files: 0–100000, seconds: 3–30, runs: 1–10')
    result=benchmark(args.baseline.resolve(),args.candidate.resolve(),args.files,args.seconds,args.runs)
    if args.output:args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'total_cpu_ms':result['median_cpu_ms'],'idle_worker_cpu_ms':result['median_idle_worker_cpu_ms']}))


if __name__=='__main__':main()
