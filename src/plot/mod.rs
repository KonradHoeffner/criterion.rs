use std::{iter, str};
use std::path::{Path, PathBuf};
use std::process::Child;

use simplot::prelude::*;
use stats::Distribution;
use stats::bivariate::Data;
use stats::bivariate::regression::Slope;
use stats::univariate::Sample;
use stats::univariate::outliers::tukey::LabeledSample;

use Estimate;
use estimate::{Distributions, Estimates, Statistic};
use {fs, kde};

pub mod both;

fn escape_underscores(string: &str) -> String {
    string.replace("_", "\\_")
}

fn scale_time(ns: f64) -> (f64, &'static str) {
    if ns < 10f64.powi(0) {
        (10f64.powi(3), "p")
    } else if ns < 10f64.powi(3) {
        (10f64.powi(0), "n")
    } else if ns < 10f64.powi(6) {
        (10f64.powi(-3), "u")
    } else if ns < 10f64.powi(9) {
        (10f64.powi(-6), "m")
    } else {
        (10f64.powi(-9), "")
    }
}

static DEFAULT_FONT: &'static str = "Helvetica";
static KDE_POINTS: usize = 500;
static SIZE: Size = Size(1280, 720);

const LINEWIDTH: LineWidth = LineWidth(2.);
const POINT_SIZE: PointSize = PointSize(0.75);

const DARK_BLUE: Color = Color::Rgb(31, 120, 180);
const DARK_ORANGE: Color = Color::Rgb(255, 127, 0);
const DARK_RED: Color = Color::Rgb(227, 26, 28);

fn debug_script(path: &PathBuf, figure: &Figure) {
    if ::debug_enabled() {
        let mut script_path = path.clone();
        script_path.set_extension("gnuplot");
        println!("Writing gnuplot script to {:?}", script_path);
        let result = figure.save(script_path.as_path());
        if let Err(e) = result {
            error!("Failed to write debug output: {}", e);
        }
    }
}

pub fn pdf_small(
    sample: &Sample<f64>,
    path: String,
    size: Option<Size>,
) -> Child {
    let path = PathBuf::from(path);
    let (x_scale, prefix) = scale_time(sample.max());
    let mean = sample.mean();

    let (xs, ys, mean_y) = kde::sweep_and_estimate(&sample, KDE_POINTS, None, mean);
    let xs_ = Sample::new(&xs);
    let ys_ = Sample::new(&ys);

    let y_limit = ys_.max() * 1.1;
    let zeros = iter::repeat(0);

    let mut figure = Figure::new();
    figure
        .set(Font(DEFAULT_FONT))
        .set(size.unwrap_or(SIZE))
        .configure(Axis::BottomX, |a| {
            a.set(Label(format!("Average time ({}s)", prefix)))
                .set(Range::Limits(xs_.min() * x_scale, xs_.max() * x_scale))
                .set(ScaleFactor(x_scale))
        })
        .configure(Axis::LeftY, |a| {
            a.set(Label("Density (a.u.)"))
                .set(Range::Limits(0., y_limit))
        })
        .configure(Axis::RightY, |a| a.hide())
        .configure(Key, |k| {
            k.hide()
        })
        .plot(
            FilledCurve {
                x: &*xs,
                y1: &*ys,
                y2: zeros,
            },
            |c| {
                c.set(Axes::BottomXRightY)
                    .set(DARK_BLUE)
                    .set(Label("PDF"))
                    .set(Opacity(0.25))
            },
        )
        .plot(
            Lines {
                x: &[mean, mean],
                y: &[0., mean_y],
            },
            |c| c.set(DARK_BLUE).set(LINEWIDTH).set(Label("Mean")),
        );

    debug_script(&path, &figure);
    figure.set(Output(path)).draw().unwrap()
}

pub fn pdf(
    data: Data<f64, f64>,
    labeled_sample: LabeledSample<f64>,
    id: &str,
    path: String,
    size: Option<Size>,
) -> Child {
    let path = PathBuf::from(path);
    let (x_scale, prefix) = scale_time(labeled_sample.max());
    let mean = labeled_sample.mean();

    let &max_iters = data.x()
        .as_slice()
        .iter()
        .max_by_key(|&&iters| iters as u64)
        .unwrap();
    let exponent = (max_iters.log10() / 3.).floor() as i32 * 3;
    let y_scale = 10f64.powi(-exponent);

    let y_label = if exponent == 0 {
        "Iterations".to_owned()
    } else {
        format!("Iterations (x 10^{})", exponent)
    };

    let (xs, ys) = kde::sweep(&labeled_sample, KDE_POINTS, None);
    let xs_ = Sample::new(&xs);

    let (lost, lomt, himt, hist) = labeled_sample.fences();

    let vertical = &[0., max_iters];
    let zeros = iter::repeat(0);

    let mut figure = Figure::new();
    figure
        .set(Font(DEFAULT_FONT))
        .set(size.unwrap_or(SIZE))
        .configure(Axis::BottomX, |a| {
            a.set(Label(format!("Average time ({}s)", prefix)))
                .set(Range::Limits(xs_.min() * x_scale, xs_.max() * x_scale))
                .set(ScaleFactor(x_scale))
        })
        .configure(Axis::LeftY, |a| {
            a.set(Label(y_label))
                .set(Range::Limits(0., max_iters * y_scale))
                .set(ScaleFactor(y_scale))
        })
        .configure(Axis::RightY, |a| {
            a.set(Label("Density (a.u.)"))
        })
        .configure(Key, |k| {
            k.set(Justification::Left)
                .set(Order::SampleText)
                .set(Position::Outside(Vertical::Top, Horizontal::Right))
        })
        .plot(
            FilledCurve {
                x: &*xs,
                y1: &*ys,
                y2: zeros,
            },
            |c| {
                c.set(Axes::BottomXRightY)
                    .set(DARK_BLUE)
                    .set(Label("PDF"))
                    .set(Opacity(0.25))
            },
        )
        .plot(
            Lines {
                x: &[mean, mean],
                y: vertical,
            },
            |c| c.set(DARK_BLUE).set(LINEWIDTH).set(LineType::Dash).set(Label("Mean"))
        )
        .plot(
            Points {
                x: labeled_sample.iter().filter_map(|(t, label)| {
                    if label.is_outlier() {
                        None
                    } else {
                        Some(t)
                    }
                }),
                y: labeled_sample
                    .iter()
                    .zip(data.x().as_slice().iter())
                    .filter_map(
                        |((_, label), i)| {
                            if label.is_outlier() {
                                None
                            } else {
                                Some(i)
                            }
                        },
                    ),
            },
            |c| {
                c.set(DARK_BLUE)
                    .set(Label("\"Clean\" sample"))
                    .set(PointType::FilledCircle)
                    .set(POINT_SIZE)
            },
        )
        .plot(
            Points {
                x: labeled_sample.iter().filter_map(
                    |(x, label)| {
                        if label.is_mild() {
                            Some(x)
                        } else {
                            None
                        }
                    },
                ),
                y: labeled_sample
                    .iter()
                    .zip(data.x().as_slice().iter())
                    .filter_map(
                        |((_, label), i)| {
                            if label.is_mild() {
                                Some(i)
                            } else {
                                None
                            }
                        },
                    ),
            },
            |c| {
                c.set(DARK_ORANGE)
                    .set(Label("Mild outliers"))
                    .set(POINT_SIZE)
                    .set(PointType::FilledCircle)
            },
        )
        .plot(
            Points {
                x: labeled_sample.iter().filter_map(
                    |(x, label)| {
                        if label.is_severe() {
                            Some(x)
                        } else {
                            None
                        }
                    },
                ),
                y: labeled_sample
                    .iter()
                    .zip(data.x().as_slice().iter())
                    .filter_map(
                        |((_, label), i)| {
                            if label.is_severe() {
                                Some(i)
                            } else {
                                None
                            }
                        },
                    ),
            },
            |c| {
                c.set(DARK_RED)
                    .set(Label("Severe outliers"))
                    .set(POINT_SIZE)
                    .set(PointType::FilledCircle)
            },
        )
        .plot(
            Lines {
                x: &[lomt, lomt],
                y: vertical,
            },
            |c| c.set(DARK_ORANGE).set(LINEWIDTH).set(LineType::Dash),
        )
        .plot(
            Lines {
                x: &[himt, himt],
                y: vertical,
            },
            |c| c.set(DARK_ORANGE).set(LINEWIDTH).set(LineType::Dash),
        )
        .plot(
            Lines {
                x: &[lost, lost],
                y: vertical,
            },
            |c| c.set(DARK_RED).set(LINEWIDTH).set(LineType::Dash),
        )
        .plot(
            Lines {
                x: &[hist, hist],
                y: vertical,
            },
            |c| c.set(DARK_RED).set(LINEWIDTH).set(LineType::Dash),
        );
    figure.set(Title(escape_underscores(id)));

    debug_script(&path, &figure);
    figure.set(Output(path)).draw().unwrap()
}

pub fn regression(
    data: Data<f64, f64>,
    point: &Slope<f64>,
    (lb, ub): (Slope<f64>, Slope<f64>),
    id: &str,
    path: String,
    size: Option<Size>,
    thumbnail_mode: bool,
) -> Child {
    let path = PathBuf::from(path);

    let (max_iters, max_elapsed) = (data.x().max(), data.y().max());

    let (y_scale, prefix) = scale_time(max_elapsed);

    let exponent = (max_iters.log10() / 3.).floor() as i32 * 3;
    let x_scale = 10f64.powi(-exponent);

    let x_label = if exponent == 0 {
        "Iterations".to_owned()
    } else {
        format!("Iterations (x 10^{})", exponent)
    };

    let lb = lb.0 * max_iters;
    let point = point.0 * max_iters;
    let ub = ub.0 * max_iters;
    let max_iters = max_iters;

    let mut figure = Figure::new();
    figure
        .set(Font(DEFAULT_FONT))
        .set(size.unwrap_or(SIZE))
        .configure(Key, |k| {
            if thumbnail_mode {
                k.hide();
            }
            k.set(Justification::Left)
                .set(Order::SampleText)
                .set(Position::Inside(Vertical::Top, Horizontal::Left))
        })
        .configure(Axis::BottomX, |a| {
            a.configure(Grid::Major, |g| g.show())
                .set(Label(x_label))
                .set(ScaleFactor(x_scale))
        })
        .configure(Axis::LeftY, |a| {
            a.configure(Grid::Major, |g| g.show())
                .set(Label(format!("Total time ({}s)", prefix)))
                .set(ScaleFactor(y_scale))
        })
        .plot(
            Points {
                x: data.x().as_slice(),
                y: data.y().as_slice(),
            },
            |c| {
                c.set(DARK_BLUE)
                    .set(Label("Sample"))
                    .set(PointSize(0.5))
                    .set(PointType::FilledCircle)
            },
        )
        .plot(
            Lines {
                x: &[0., max_iters],
                y: &[0., point],
            },
            |c| {
                c.set(DARK_BLUE)
                    .set(LINEWIDTH)
                    .set(Label("Linear regression"))
                    .set(LineType::Solid)
            },
        )
        .plot(
            FilledCurve {
                x: &[0., max_iters],
                y1: &[0., lb],
                y2: &[0., ub],
            },
            |c| {
                c.set(DARK_BLUE)
                    .set(Label("Confidence interval"))
                    .set(Opacity(0.25))
            },
        );
    if !thumbnail_mode {
        figure.set(Title(escape_underscores(id)));
    }

    debug_script(&path, &figure);
    figure.set(Output(path)).draw().unwrap()
}

pub(crate) fn abs_distributions(
    distributions: &Distributions,
    estimates: &Estimates,
    id: &str,
    output_directory: &str,
) -> Vec<Child> {
    distributions
        .iter()
        .map(|(&statistic, distribution)| {
            let path = PathBuf::from(format!("{}/{}/new/{}.svg", output_directory, id, statistic));
            let estimate = estimates[&statistic];

            let ci = estimate.confidence_interval;
            let (lb, ub) = (ci.lower_bound, ci.upper_bound);

            let start = lb - (ub - lb) / 9.;
            let end = ub + (ub - lb) / 9.;
            let (xs, ys) = kde::sweep(distribution, KDE_POINTS, Some((start, end)));
            let xs_ = Sample::new(&xs);

            let (x_scale, prefix) = scale_time(xs_.max());
            let y_scale = x_scale.recip();

            let p = estimate.point_estimate;

            let n_p = xs.iter().enumerate().find(|&(_, &x)| x >= p).unwrap().0;
            let y_p =
                ys[n_p - 1] + (ys[n_p] - ys[n_p - 1]) / (xs[n_p] - xs[n_p - 1]) * (p - xs[n_p - 1]);

            let zero = iter::repeat(0);

            let start = xs.iter().enumerate().find(|&(_, &x)| x >= lb).unwrap().0;
            let end = xs.iter()
                .enumerate()
                .rev()
                .find(|&(_, &x)| x <= ub)
                .unwrap()
                .0;
            let len = end - start;

            let mut figure = Figure::new();
            figure
                .set(Font(DEFAULT_FONT))
                .set(SIZE)
                .set(Title(format!("{}: {}", escape_underscores(id), statistic)))
                .configure(Axis::BottomX, |a| {
                    a.set(Label(format!("Average time ({}s)", prefix)))
                        .set(Range::Limits(xs_.min() * x_scale, xs_.max() * x_scale))
                        .set(ScaleFactor(x_scale))
                })
                .configure(Axis::LeftY, |a| {
                    a.set(Label("Density (a.u.)")).set(ScaleFactor(y_scale))
                })
                .configure(Key, |k| {
                    k.set(Justification::Left)
                        .set(Order::SampleText)
                        .set(Position::Outside(Vertical::Top, Horizontal::Right))
                })
                .plot(Lines { x: &*xs, y: &*ys }, |c| {
                    c.set(DARK_BLUE)
                        .set(LINEWIDTH)
                        .set(Label("Bootstrap distribution"))
                        .set(LineType::Solid)
                })
                .plot(
                    FilledCurve {
                        x: xs.iter().skip(start).take(len),
                        y1: ys.iter().skip(start),
                        y2: zero,
                    },
                    |c| {
                        c.set(DARK_BLUE)
                            .set(Label("Confidence interval"))
                            .set(Opacity(0.25))
                    },
                )
                .plot(
                    Lines {
                        x: &[p, p],
                        y: &[0., y_p],
                    },
                    |c| {
                        c.set(DARK_BLUE)
                            .set(LINEWIDTH)
                            .set(Label("Point estimate"))
                            .set(LineType::Dash)
                    },
                );
            debug_script(&path, &figure);
            figure.set(Output(path)).draw().unwrap()
        })
        .collect::<Vec<_>>()
}

// TODO DRY: This is very similar to the `abs_distributions` method
pub(crate) fn rel_distributions(
    distributions: &Distributions,
    estimates: &Estimates,
    id: &str,
    output_directory: &str,
    nt: f64,
) -> Vec<Child> {
    let mut figure = Figure::new();

    figure
        .set(Font(DEFAULT_FONT))
        .set(SIZE)
        .configure(Axis::LeftY, |a| a.set(Label("Density (a.u.)")))
        .configure(Key, |k| {
            k.set(Justification::Left)
                .set(Order::SampleText)
                .set(Position::Outside(Vertical::Top, Horizontal::Right))
        });

    distributions
        .iter()
        .map(|(&statistic, distribution)| {
            let path = PathBuf::from(format!(
                "{}/{}/change/{}.svg",
                output_directory, id, statistic
            ));

            let estimate = estimates[&statistic];
            let ci = estimate.confidence_interval;
            let (lb, ub) = (ci.lower_bound, ci.upper_bound);

            let start = lb - (ub - lb) / 9.;
            let end = ub + (ub - lb) / 9.;
            let (xs, ys) = kde::sweep(distribution, KDE_POINTS, Some((start, end)));
            let xs_ = Sample::new(&xs);

            let p = estimate.point_estimate;
            let n_p = xs.iter().enumerate().find(|&(_, &x)| x >= p).unwrap().0;
            let y_p =
                ys[n_p - 1] + (ys[n_p] - ys[n_p - 1]) / (xs[n_p] - xs[n_p - 1]) * (p - xs[n_p - 1]);

            let one = iter::repeat(1);
            let zero = iter::repeat(0);

            let start = xs.iter().enumerate().find(|&(_, &x)| x >= lb).unwrap().0;
            let end = xs.iter()
                .enumerate()
                .rev()
                .find(|&(_, &x)| x <= ub)
                .unwrap()
                .0;
            let len = end - start;

            let x_min = xs_.min();
            let x_max = xs_.max();

            let (fc_start, fc_end) = if nt < x_min || -nt > x_max {
                let middle = (x_min + x_max) / 2.;

                (middle, middle)
            } else {
                (
                    if -nt < x_min { x_min } else { -nt },
                    if nt > x_max { x_max } else { nt },
                )
            };

            let mut figure = figure.clone();
            figure
                .set(Title(format!("{}: {}", escape_underscores(id), statistic)))
                .configure(Axis::BottomX, |a| {
                    a.set(Label("Relative change (%)"))
                        .set(Range::Limits(x_min * 100., x_max * 100.))
                        .set(ScaleFactor(100.))
                })
                .plot(Lines { x: &*xs, y: &*ys }, |c| {
                    c.set(DARK_BLUE)
                        .set(LINEWIDTH)
                        .set(Label("Bootstrap distribution"))
                        .set(LineType::Solid)
                })
                .plot(
                    FilledCurve {
                        x: xs.iter().skip(start).take(len),
                        y1: ys.iter().skip(start),
                        y2: zero.clone(),
                    },
                    |c| {
                        c.set(DARK_BLUE)
                            .set(Label("Confidence interval"))
                            .set(Opacity(0.25))
                    },
                )
                .plot(
                    Lines {
                        x: &[p, p],
                        y: &[0., y_p],
                    },
                    |c| {
                        c.set(DARK_BLUE)
                            .set(LINEWIDTH)
                            .set(Label("Point estimate"))
                            .set(LineType::Dash)
                    },
                )
                .plot(
                    FilledCurve {
                        x: &[fc_start, fc_end],
                        y1: one,
                        y2: zero,
                    },
                    |c| {
                        c.set(Axes::BottomXRightY)
                            .set(DARK_RED)
                            .set(Label("Noise threshold"))
                            .set(Opacity(0.1))
                    },
                );
            debug_script(&path, &figure);
            figure.set(Output(path)).draw().unwrap()
        })
        .collect::<Vec<_>>()
}

pub fn t_test(t: f64, distribution: &Distribution<f64>, id: &str, output_directory: &str) -> Child {
    let path = PathBuf::from(format!("{}/{}/change/t-test.svg", output_directory, id));

    let (xs, ys) = kde::sweep(distribution, KDE_POINTS, None);
    let zero = iter::repeat(0);

    let mut figure = Figure::new();
    figure
        .set(Font(DEFAULT_FONT))
        .set(SIZE)
        .set(Title(format!("{}: Welch t test", escape_underscores(id))))
        .configure(Axis::BottomX, |a| a.set(Label("t score")))
        .configure(Axis::LeftY, |a| a.set(Label("Density")))
        .configure(Key, |k| {
            k.set(Justification::Left)
                .set(Order::SampleText)
                .set(Position::Outside(Vertical::Top, Horizontal::Right))
        })
        .plot(
            FilledCurve {
                x: &*xs,
                y1: &*ys,
                y2: zero,
            },
            |c| {
                c.set(DARK_BLUE)
                    .set(Label("t distribution"))
                    .set(Opacity(0.25))
            },
        )
        .plot(
            Lines {
                x: &[t, t],
                y: &[0, 1],
            },
            |c| {
                c.set(Axes::BottomXRightY)
                    .set(DARK_BLUE)
                    .set(LINEWIDTH)
                    .set(Label("t statistic"))
                    .set(LineType::Solid)
            },
        );
    debug_script(&path, &figure);
    figure.set(Output(path)).draw().unwrap()
}

/// Private
trait Append<T> {
    /// Private
    fn append_(self, item: T) -> Self;
}

// NB I wish this was in the standard library
impl<T> Append<T> for Vec<T> {
    fn append_(mut self, item: T) -> Vec<T> {
        self.push(item);
        self
    }
}

pub fn summarize(group_id: &str, all_ids: &[String], output_directory: &str) -> Vec<Child> {
    let output_dir = Path::new(output_directory);
    let dir = output_dir.join(group_id);
    let contents: Vec<_> = all_ids.iter().map(|id| output_dir.join(id)).collect();

    let mut all_gnuplots = vec![];

    // XXX Plot both summaries?
    for &sample in &["new", "base"] {
        let mut benches = contents
            .iter()
            .filter_map(|entry| {
                if entry.is_dir() && entry.file_name().and_then(|s| s.to_str()) != Some("summary") {
                    let label = entry.file_name().unwrap().to_str().unwrap();
                    let root = entry.join(sample);

                    if let Some(estimates) = Estimate::load(&root.join("estimates.json")) {
                        let (iters, times): (Vec<f64>, Vec<f64>) =
                            try_else_return!(fs::load(&root.join("sample.json")), || None);
                        let avg_times = iters
                            .into_iter()
                            .zip(times.into_iter())
                            .map(|(iters, time)| time / iters)
                            .collect::<Vec<_>>();

                        Some((label, label.parse::<usize>(), estimates, avg_times))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if benches.len() < 2 {
            continue;
        }

        // The following code needs this directory,
        // therefore we exit the function if anything goes wrong.
        try_else_return!(
            fs::mkdirp(&dir.join(&format!("summary/{}", sample))),
            || vec![]
        );

        let gnuplots = if benches.iter().all(|&(_, ref input, _, _)| input.is_ok()) {
            // TODO trendline
            let mut benches = benches
                .into_iter()
                .map(|(label, input, estimates, sample)| (label, input.unwrap(), estimates, sample))
                .collect::<Vec<_>>();

            benches.sort_by(|&(_, a, _, _), &(_, b, _, _)| a.cmp(&b));

            [Statistic::Mean, Statistic::Median, Statistic::Slope]
                .iter()
                .map(|&statistic| {
                    let points = benches
                        .iter()
                        .map(|&(_, _, ref estimates, _)| estimates[&statistic].point_estimate)
                        .collect::<Vec<_>>();
                    let lbs = benches
                        .iter()
                        .map(|&(_, _, ref estimates, _)| {
                            estimates[&statistic].confidence_interval.lower_bound
                        })
                        .collect::<Vec<_>>();
                    let ubs = benches
                        .iter()
                        .map(|&(_, _, ref estimates, _)| {
                            estimates[&statistic].confidence_interval.upper_bound
                        })
                        .collect::<Vec<_>>();
                    let ubs_ = Sample::new(&ubs);

                    // XXX scale inputs?
                    let inputs = benches
                        .iter()
                        .map(|&(_, input, _, _)| input)
                        .collect::<Vec<_>>();
                    let (scale, prefix) = scale_time(ubs_.max());

                    let path = dir.join(&format!("summary/{}/{}s.svg", sample, statistic));
                    // TODO Review axis scaling
                    let mut figure = Figure::new();
                    figure
                        .set(Font(DEFAULT_FONT))
                        .set(SIZE)
                        .set(Title(escape_underscores(group_id)))
                        .configure(Axis::BottomX, |a| {
                            a.configure(Grid::Major, |g| g.show())
                                .configure(Grid::Minor, |g| g.hide())
                                .set(Label("Input"))
                                .set(Scale::Linear)
                        })
                        .configure(Axis::LeftY, |a| {
                            a.configure(Grid::Major, |g| g.show())
                                .configure(Grid::Minor, |g| g.hide())
                                .set(Label(format!("Average time ({}s)", prefix)))
                                .set(Scale::Linear)
                                .set(ScaleFactor(scale))
                        })
                        .configure(Key, |k| {
                            k.set(Justification::Left)
                                .set(Order::SampleText)
                                .set(Position::Inside(Vertical::Top, Horizontal::Left))
                        })
                        .plot(
                            YErrorBars {
                                x: &*inputs,
                                y: &*points,
                                y_low: &*lbs,
                                y_high: &*ubs,
                            },
                            |e| {
                                e.set(LINEWIDTH)
                                    .set(Label(format!("{}", statistic)))
                                    .set(POINT_SIZE)
                                    .set(PointType::FilledCircle)
                            },
                        );
                    debug_script(&path, &figure);
                    figure.set(Output(path)).draw().unwrap()
                })
                .collect::<Vec<_>>()
        } else {
            // NB median go last because we reuse the ordered set in the next step (summary)
            [Statistic::Mean, Statistic::Slope, Statistic::Median]
                .iter()
                .map(|&statistic| {
                    benches.sort_by(|&(_, _, ref a, _), &(_, _, ref b, _)| {
                        let a = a[&statistic].point_estimate;
                        let b = b[&statistic].point_estimate;
                        b.partial_cmp(&a).unwrap()
                    });

                    let points = benches
                        .iter()
                        .map(|&(_, _, ref estimates, _)| estimates[&statistic].point_estimate)
                        .collect::<Vec<_>>();
                    let lbs = benches
                        .iter()
                        .map(|&(_, _, ref estimates, _)| {
                            estimates[&statistic].confidence_interval.lower_bound
                        })
                        .collect::<Vec<_>>();
                    let ubs = benches
                        .iter()
                        .map(|&(_, _, ref estimates, _)| {
                            estimates[&statistic].confidence_interval.upper_bound
                        })
                        .collect::<Vec<_>>();
                    let ubs_ = Sample::new(&ubs);

                    let (scale, prefix) = scale_time(ubs_.max());

                    let min = *points.last().unwrap();
                    let rel = points
                        .iter()
                        .map(|&x| format!("{:.02}", x / min))
                        .collect::<Vec<_>>();

                    let tics = || (0..).map(|x| (f64::from(x)) + 0.5);
                    let path = dir.join(&format!("summary/{}/{}s.svg", sample, statistic));
                    let mut figure = Figure::new();
                    figure
                        .set(Font(DEFAULT_FONT))
                        .set(SIZE)
                        .set(Title(format!(
                            "{}: Estimates of the {}s",
                            escape_underscores(group_id),
                            statistic
                        )))
                        .configure(Axis::BottomX, |a| {
                            a.configure(Grid::Major, |g| g.show())
                                .configure(Grid::Minor, |g| g.hide())
                                .set(Label(format!("Average time ({}s)", prefix)))
                                .set(Scale::Linear)
                                .set(ScaleFactor(scale))
                        })
                        .configure(Axis::BottomX, |a| a)
                        .configure(Axis::LeftY, |a| {
                            a.set(Label("Input"))
                                .set(Range::Limits(0., benches.len() as f64))
                                .set(TicLabels {
                                    positions: tics(),
                                    labels: benches.iter().map(|&(label, _, _, _)| label),
                                })
                        })
                        .configure(Axis::RightY, |a| {
                            a.set(Label("Relative time"))
                                .set(Range::Limits(0., benches.len() as f64))
                                .set(TicLabels {
                                    positions: tics(),
                                    labels: rel.iter(),
                                })
                        })
                        .plot(
                            XErrorBars {
                                x: &*points,
                                y: tics(),
                                x_low: &*lbs,
                                x_high: &*ubs,
                            },
                            |eb| {
                                eb.set(LINEWIDTH)
                                    .set(Label("Confidence Interval"))
                                    .set(POINT_SIZE)
                                    .set(PointType::FilledCircle)
                            },
                        );
                    debug_script(&path, &figure);
                    figure.set(Output(path)).draw().unwrap()
                })
                .collect::<Vec<_>>()
                .append_({
                    let kdes = benches
                        .iter()
                        .map(|&(_, _, _, ref sample)| {
                            let (x, mut y) = kde::sweep(Sample::new(sample), KDE_POINTS, None);
                            let y_max = Sample::new(&y).max();
                            for y in y.iter_mut() {
                                *y /= y_max;
                            }

                            (x, y)
                        })
                        .collect::<Vec<_>>();
                    let medians = benches
                        .iter()
                        .map(|&(_, _, _, ref sample)| Sample::new(sample).percentiles().median())
                        .collect::<Vec<_>>();
                    let mut xs = kdes.iter()
                        .flat_map(|&(ref x, _)| x.iter())
                        .filter(|&&x| x > 0.);
                    let (mut min, mut max) = {
                        let &first = xs.next().unwrap();
                        (first, first)
                    };
                    for &e in xs {
                        if e < min {
                            min = e;
                        } else if e > max {
                            max = e;
                        }
                    }
                    let (scale, prefix) = scale_time(max);

                    let tics = || (0..).map(|x| (f64::from(x)) + 0.5);
                    let path = dir.join(&format!("summary/{}/violin_plot.svg", sample));
                    let mut f = Figure::new();
                    f.set(Font(DEFAULT_FONT))
                        .set(SIZE)
                        .set(Title(format!(
                            "{}: Violin plot",
                            escape_underscores(group_id)
                        )))
                        .configure(Axis::BottomX, |a| {
                            a.configure(Grid::Major, |g| g.show())
                                .configure(Grid::Minor, |g| g.hide())
                                .set(Label(format!("Average time ({}s)", prefix)))
                                .set(Scale::Linear)
                                .set(ScaleFactor(scale))
                        })
                        .configure(Axis::BottomX, |a| a)
                        .configure(Axis::LeftY, |a| {
                            a.set(Label("Input"))
                                .set(Range::Limits(0., benches.len() as f64))
                                .set(TicLabels {
                                    positions: tics(),
                                    labels: benches.iter().map(|&(label, _, _, _)| label),
                                })
                        })
                        .plot(
                            Points {
                                x: medians.iter().cloned(),
                                y: tics(),
                            },
                            |c| {
                                c.set(Color::Black)
                                    .set(Label("Median"))
                                    .set(PointType::Plus)
                                    .set(PointSize(2. * POINT_SIZE.0))
                            },
                        );

                    let mut is_first = true;
                    for (i, &(ref x, ref y)) in kdes.iter().enumerate() {
                        let i = i as f64 + 0.5;
                        let y1 = y.iter().map(|&y| i + y * 0.5);
                        let y2 = y.iter().map(|&y| i - y * 0.5);

                        f.plot(
                            FilledCurve {
                                x: &**x,
                                y1: y1,
                                y2: y2,
                            },
                            |c| {
                                if is_first {
                                    is_first = false;

                                    c.set(DARK_BLUE).set(Label("PDF")).set(Opacity(0.25))
                                } else {
                                    c.set(DARK_BLUE).set(Opacity(0.25))
                                }
                            },
                        );
                    }
                    debug_script(&path, &f);
                    f.set(Output(path)).draw().unwrap()
                })
        };

        all_gnuplots.extend(gnuplots);
    }
    all_gnuplots
}
