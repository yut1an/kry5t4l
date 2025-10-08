use std::net::SocketAddr;

use kry5t4l_share::modules::{protocol::{get_cur_timestamp_secs, Message}, screen::{DiffBlock, ScreenFrame}};
use lz4_flex::block;

use crate::views::monitor::{send_monitor_update, MonitorUpdate};

pub fn handle_screenshot_data(msg: Message) {
    let data = msg.content();

    if data.len() < 10 {
        println!("屏幕数据太短，忽略");
        return;
    }

    // 数据格式: [frame_type(1)] + [width(4)] + [height(4)] + [数据...]
    let frame_type = data[0]; // 0=完整帧, 1=差分帧
    let width = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let height = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);

    let is_full_frame = frame_type == 0;

    // println!("收到{}帧数据: {}x{}, 数据大小: {} bytes", 
    //     if is_full_frame { "完整" } else { "差分" },
    //     width, height, data.len());

    if is_full_frame {
        handle_full_frame(msg.clientid(), width, height, &data[9..]);
    } else {
        handle_diff_frame(msg.clientid(), width, height, &data[9..]);
    }

}


fn handle_full_frame(client_id: String, width: u32, height: u32, compressed_data: &[u8]) {
    // 完整帧数据已经是 LZ4 压缩的
    //println!("处理完整帧: {}x{}, 压缩数据大小: {} bytes", width, height, compressed_data.len());

    // 首次发送屏幕信息（用于初始化窗口大小）
    send_monitor_update(MonitorUpdate::ScreenInfo { 
        client_id: client_id.clone(), 
        width, 
        height,
    });

    // 发送完整屏幕数据（保持压缩状态，在客户端解压）
    let screen_frame = ScreenFrame {
        frame_id: get_cur_timestamp_secs(),
        timestamp: get_cur_timestamp_secs(),
        is_full_frame: true,
        width,
        height,
        data: compressed_data.to_vec(), // LZ4 压缩的 RGBA 数据
        diff_blocks: vec![],
    };

    send_monitor_update(MonitorUpdate::ScreenData { 
        client_id, 
        screen_data: screen_frame,
    });

}

fn handle_diff_frame(client_id: String, width: u32, height: u32, diff_data: &[u8]) {
    let diff_blocks = parse_diff_blocks(diff_data);

    if diff_blocks.is_empty() {
        println!("没有差分块");
        return;
    }

    //println!("处理差分帧: {} 个差分块", diff_blocks.len());

    let screen_frame = ScreenFrame {
        frame_id: get_cur_timestamp_secs(),
        timestamp: get_cur_timestamp_secs(),
        is_full_frame: false,
        width,
        height,
        data: vec![],
        diff_blocks,
    };

    send_monitor_update(MonitorUpdate::ScreenData { 
        client_id, 
        screen_data: screen_frame 
    });

}

// 解析差分块数据
// 格式: [block_count(4字节)] + [block1] + [block2] + ...
// 每个block: [x(4)][y(4)][width(4)][height(4)][compressed_data_len(4)][compressed_data...]
fn parse_diff_blocks(data: &[u8]) -> Vec<DiffBlock> {
    let mut blocks = Vec::new();

    if data.len() < 4 {
        return blocks;
    }

    let block_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut offset = 4;

    //println!("解析差分块，预期数量: {}", block_count);

    for i in 0..block_count {
        if offset + 20 > data.len() {
             println!("数据不足，无法解析第 {} 个块", i);
             break;
        }
        
        let x = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
        let y = u32::from_le_bytes([data[offset+4], data[offset+5], data[offset+6], data[offset+7]]);
        let width = u32::from_le_bytes([data[offset+8], data[offset+9], data[offset+10], data[offset+11]]);
        let height = u32::from_le_bytes([data[offset+12], data[offset+13], data[offset+14], data[offset+15]]);
        let data_len = u32::from_le_bytes([data[offset+16], data[offset+17], data[offset+18], data[offset+19]]) as usize;

        offset += 20;

        if offset + data_len > data.len() {
            println!("数据长度不足，期望 {}, 剩余 {}", data_len, data.len() - offset);
            break;
        }

        let compressed_block_data = data[offset..offset + data_len].to_vec();
        offset += data_len;

        blocks.push(DiffBlock { 
            x, 
            y, 
            width, 
            height, 
            data: compressed_block_data // LZ4 压缩的 RGBA 数据
        });
        
        // println!("解析块 {}: 位置({}, {}), 大小({}x{}), 压缩数据: {} bytes", 
        //     i, x, y, width, height, data_len);
    }

    //println!("成功解析 {} 个差分块", blocks.len());
    blocks
}