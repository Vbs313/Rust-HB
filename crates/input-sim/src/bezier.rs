//! 贝塞尔曲线路径生成
//!
//! 模拟人类鼠标移动的自然曲线轨迹。
//! 基于三次贝塞尔曲线，加入随机扰动使轨迹更自然。

/// 二维坐标点
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// 生成从起点到终点的贝塞尔曲线路径
///
/// `steps`: 路径分段数（控制精细度）
pub fn generate_bezier_path(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Vec<Point> {
    let start = Point::new(from_x as f64, from_y as f64);
    let end = Point::new(to_x as f64, to_y as f64);

    // 生成两个随机控制点，模拟人类手臂的自然轨迹
    let dist = distance(&start, &end);
    let control_dist = dist * 0.3 + rand::random::<f64>() * dist * 0.2;

    let angle = (end.y - start.y).atan2(end.x - start.x);
    let control_angle1 = angle + (rand::random::<f64>() - 0.5) * 1.2;
    let control_angle2 = angle + (rand::random::<f64>() - 0.5) * 1.2;

    let cp1 = Point::new(
        start.x + control_dist * control_angle1.cos(),
        start.y + control_dist * control_angle1.sin(),
    );
    let cp2 = Point::new(
        end.x - control_dist * control_angle2.cos(),
        end.y - control_dist * control_angle2.sin(),
    );

    // 计算曲线上的点
    let steps = (dist / 5.0).max(10.0).min(100.0) as usize;
    let mut points = Vec::with_capacity(steps);

    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let point = cubic_bezier(&start, &cp1, &cp2, &end, t);

        // 加入微小随机扰动
        let jitter = 0.5;
        let x = point.x + (rand::random::<f64>() - 0.5) * jitter;
        let y = point.y + (rand::random::<f64>() - 0.5) * jitter;

        points.push(Point::new(x, y));
    }

    points
}

/// 三次贝塞尔曲线计算
fn cubic_bezier(p0: &Point, p1: &Point, p2: &Point, p3: &Point, t: f64) -> Point {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;

    Point::new(
        uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x,
        uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y,
    )
}

/// 两点间距离
fn distance(a: &Point, b: &Point) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bezier_path_generates_correct_length() {
        let points = generate_bezier_path(100, 100, 500, 500);
        assert!(points.len() >= 10);
        assert!(points.len() <= 500);

        // 第一个点应接近起点
        assert!((points[0].x - 100.0).abs() < 2.0);
        assert!((points[0].y - 100.0).abs() < 2.0);

        // 最后一个点应接近终点
        assert!((points[points.len() - 1].x - 500.0).abs() < 2.0);
        assert!((points[points.len() - 1].y - 500.0).abs() < 2.0);
    }

    #[test]
    fn test_bezier_smoothness() {
        let points = generate_bezier_path(0, 0, 1000, 1000);
        // 路径应单调递增（起点到终点）
        for i in 1..points.len() {
            assert!(points[i].x >= points[i - 1].x || (points[i].x - points[i - 1].x).abs() < 5.0);
        }
    }
}
